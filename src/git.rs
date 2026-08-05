pub mod branching {
    use chrono::{DateTime, Utc};
    use clap::Parser;
    use git2::{BranchType, ErrorCode, Repository, StashFlags, Status};
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};
    use std::time::Duration;
    use timeago::Formatter;

    #[derive(Debug, Eq, PartialEq, Clone)]
    pub struct BranchInfo {
        /// Local name, or remote name like "origin/feature" when `is_remote`.
        pub branch_name: String,
        pub last_commit_time: i64,
        pub time_ago: String,
        /// Tip commit summary line.
        pub summary: String,
        pub is_head: bool,
        pub remote_tracking: Option<String>,
        /// True for a remote branch with no local counterpart.
        pub is_remote: bool,
        /// True when a githist stash created from this branch is pending.
        pub has_stash: bool,
        /// Reflog-derived checkout recency; higher = more recently checked out.
        pub checkout_rank: Option<i64>,
        /// Absolute path of another worktree that has this branch checked out.
        pub worktree_path: Option<String>,
    }

    fn commit_fields(
        commit: &git2::Commit,
        formatter: &Formatter,
        now: DateTime<Utc>,
    ) -> (i64, String, String) {
        let last_commit_time = commit.time().seconds();
        let time_ago = DateTime::from_timestamp(last_commit_time, 0).map_or_else(
            || "unknown".to_string(),
            |dt| formatter.convert_chrono(dt, now),
        );
        let summary = commit.summary().ok().flatten().unwrap_or("").to_string();
        (last_commit_time, time_ago, summary)
    }

    /// A TUI for quickly switching between recent Git branches
    #[derive(Parser, Debug)]
    #[command(version, about)]
    pub struct Config {
        /// Path to the git repository
        #[arg(default_value = ".")]
        pub repo_path: String,

        /// UI tick rate in milliseconds
        #[arg(long, default_value_t = 250, hide = true)]
        pub tick_rate_ms: u64,
    }

    impl Config {
        pub fn tick_rate(&self) -> Duration {
            Duration::from_millis(self.tick_rate_ms)
        }
    }

    /// Wrapper around a git2::Repository to avoid re-opening on every operation.
    pub struct Repo {
        inner: Repository,
    }

    const STASH_MARKER: &str = "githist: stash before switching from ";

    /// Local branch name for a remote branch like "origin/feature".
    #[must_use]
    pub fn local_branch_name(remote_name: &str) -> &str {
        remote_name
            .split_once('/')
            .map_or(remote_name, |(_, suffix)| suffix)
    }

    const STASHABLE_STATUS: Status = Status::INDEX_NEW
        .union(Status::INDEX_MODIFIED)
        .union(Status::INDEX_DELETED)
        .union(Status::INDEX_RENAMED)
        .union(Status::INDEX_TYPECHANGE)
        .union(Status::WT_MODIFIED)
        .union(Status::WT_DELETED)
        .union(Status::WT_RENAMED)
        .union(Status::WT_TYPECHANGE)
        .union(Status::WT_NEW);

    fn is_stashable_status(status: Status) -> bool {
        status.intersects(STASHABLE_STATUS) && !status.contains(Status::IGNORED)
    }

    fn normalize_workdir(path: &Path) -> PathBuf {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        canonical.components().as_path().to_path_buf()
    }

    fn head_branch_of(repo: &Repository) -> Option<String> {
        let head = repo.head().ok()?;
        if head.is_branch() {
            head.shorthand().ok().map(String::from)
        } else {
            None
        }
    }

    impl Repo {
        /// # Errors
        ///
        /// Will return `git2::Error` if not a valid repo.
        pub fn open(config: &Config) -> Result<Repo, git2::Error> {
            let inner = Repository::open(&config.repo_path)?;
            Ok(Repo { inner })
        }

        /// Absolute path of this repository's working tree (or `.git` for bare repos).
        #[must_use]
        pub fn workdir_path(&self) -> PathBuf {
            if let Some(wd) = self.inner.workdir() {
                normalize_workdir(wd)
            } else {
                normalize_workdir(self.inner.path())
            }
        }

        /// URL of the `origin` remote, if configured.
        #[must_use]
        pub fn remote_origin_url(&self) -> Option<String> {
            self.inner
                .find_remote("origin")
                .ok()
                .and_then(|remote| remote.url().ok().map(String::from))
        }

        /// Stable per-repo identifier for agent link storage.
        #[must_use]
        pub fn repo_id(&self) -> String {
            if let Some(url) = self.remote_origin_url() {
                crate::agent_store::repo_id_from_remote(&url)
            } else {
                crate::agent_store::repo_id_from_path(&self.workdir_path())
            }
        }

        /// Returns the name of the current HEAD branch, or None if detached.
        fn head_branch_name(&self) -> Option<String> {
            head_branch_of(&self.inner)
        }

        /// Map branch name -> checkout recency rank parsed from the HEAD reflog.
        /// Higher rank = more recently checked out. Rank is reflog position
        /// (not timestamp) so same-second checkouts stay ordered.
        fn checkout_recency(&self) -> HashMap<String, i64> {
            let mut map = HashMap::new();
            let Ok(reflog) = self.inner.reflog("HEAD") else {
                return map;
            };
            let total = i64::try_from(reflog.len()).unwrap_or(i64::MAX);
            for (idx, entry) in reflog.iter().enumerate() {
                let Ok(Some(msg)) = entry.message() else {
                    continue;
                };
                let Some(rest) = msg.strip_prefix("checkout: moving from ") else {
                    continue;
                };
                let Some((_, to)) = rest.split_once(" to ") else {
                    continue;
                };
                let rank = total - i64::try_from(idx).unwrap_or(0);
                map.entry(to.to_string()).or_insert(rank);
            }
            map
        }

        /// The branch checked out before the current one, like `git checkout -`.
        #[must_use]
        pub fn previous_branch(&self) -> Option<String> {
            let current = self.head_branch_name();
            let reflog = self.inner.reflog("HEAD").ok()?;
            for entry in reflog.iter() {
                let Ok(Some(msg)) = entry.message() else {
                    continue;
                };
                let Some(rest) = msg.strip_prefix("checkout: moving from ") else {
                    continue;
                };
                let Some((from, _)) = rest.split_once(" to ") else {
                    continue;
                };
                if Some(from) != current.as_deref()
                    && self.inner.find_branch(from, BranchType::Local).is_ok()
                {
                    return Some(from.to_string());
                }
            }
            None
        }

        /// Compute ahead/behind info relative to the remote tracking branch.
        fn remote_tracking_info(&self, branch_name: &str) -> Option<String> {
            let branch = self
                .inner
                .find_branch(branch_name, BranchType::Local)
                .ok()?;
            let upstream = branch.upstream().ok()?;
            let local_oid = branch.get().target()?;
            let upstream_oid = upstream.get().target()?;
            let (ahead, behind) = self
                .inner
                .graph_ahead_behind(local_oid, upstream_oid)
                .ok()?;
            if ahead == 0 && behind == 0 {
                Some("up to date".to_string())
            } else {
                let mut parts = Vec::new();
                if ahead > 0 {
                    parts.push(format!("+{ahead}"));
                }
                if behind > 0 {
                    parts.push(format!("-{behind}"));
                }
                Some(parts.join("/"))
            }
        }

        /// # Errors
        ///
        /// Will return `git2::Error` if not a valid repo.
        pub fn get_branch_names(&mut self) -> Result<Vec<BranchInfo>, git2::Error> {
            let mut result = Vec::new();
            let head_name = self.head_branch_name();
            let recency = self.checkout_recency();
            let stashed = self.stashed_branches();
            let worktree_paths = self.other_worktree_paths()?;
            let branches = self.inner.branches(Some(BranchType::Local))?;
            let formatter = Formatter::new();
            let now = Utc::now();
            let mut local_names = HashSet::new();

            for branch in branches {
                let (branch, _) = branch?;
                let Some(branch_name) = branch.name()?.map(String::from) else {
                    continue; // skip branches with non-UTF8 names
                };
                local_names.insert(branch_name.clone());
                let last_commit = branch.get().peel_to_commit()?;
                let (last_commit_time, time_ago, summary) =
                    commit_fields(&last_commit, &formatter, now);
                let is_head = head_name.as_deref() == Some(branch_name.as_str());
                let remote_tracking = self.remote_tracking_info(&branch_name);
                let checkout_rank = recency.get(&branch_name).copied();
                let has_stash = stashed.contains(&branch_name);
                let worktree_path = worktree_paths.get(&branch_name).cloned();

                result.push(BranchInfo {
                    branch_name,
                    last_commit_time,
                    time_ago,
                    summary,
                    is_head,
                    remote_tracking,
                    is_remote: false,
                    has_stash,
                    checkout_rank,
                    worktree_path,
                });
            }

            for branch in self.inner.branches(Some(BranchType::Remote))? {
                let (branch, _) = branch?;
                let Some(branch_name) = branch.name()?.map(String::from) else {
                    continue;
                };
                if branch_name.ends_with("/HEAD") {
                    continue;
                }
                if local_names.contains(local_branch_name(&branch_name)) {
                    continue;
                }
                let last_commit = branch.get().peel_to_commit()?;
                let (last_commit_time, time_ago, summary) =
                    commit_fields(&last_commit, &formatter, now);
                result.push(BranchInfo {
                    branch_name,
                    last_commit_time,
                    time_ago,
                    summary,
                    is_head: false,
                    remote_tracking: None,
                    is_remote: true,
                    has_stash: false,
                    checkout_rank: None,
                    worktree_path: None,
                });
            }

            result.sort_by(|a, b| {
                let ka = (a.checkout_rank.unwrap_or(i64::MIN), a.last_commit_time);
                let kb = (b.checkout_rank.unwrap_or(i64::MIN), b.last_commit_time);
                kb.cmp(&ka)
            });
            Ok(result)
        }

        /// Returns true if there are local changes that can be stashed.
        ///
        /// # Errors
        ///
        /// Will return `git2::Error` if status could not be read.
        pub fn is_dirty(&self) -> Result<bool, git2::Error> {
            let mut opts = git2::StatusOptions::new();
            opts.exclude_submodules(true);
            opts.include_untracked(true);
            let statuses = self.inner.statuses(Some(&mut opts))?;
            Ok(statuses
                .iter()
                .any(|entry| is_stashable_status(entry.status())))
        }

        /// Stash tracked and untracked local changes.
        ///
        /// # Errors
        ///
        /// Will return `git2::Error` if stashing failed.
        pub fn stash_changes(&mut self, target_branch: &str) -> Result<(), git2::Error> {
            let current = self
                .head_branch_name()
                .unwrap_or_else(|| "HEAD".to_string());
            let message = format!("{STASH_MARKER}{current} to {target_branch}");
            let signature = self.inner.signature()?;
            match self
                .inner
                .stash_save(&signature, &message, Some(StashFlags::INCLUDE_UNTRACKED))
            {
                Ok(_) => Ok(()),
                Err(error) if error.code() == ErrorCode::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }

        /// Branch names that have a pending githist stash (parsed from stash
        /// messages; git prefixes them with "On <branch>: ").
        fn stashed_branches(&mut self) -> HashSet<String> {
            let mut set = HashSet::new();
            let _ = self.inner.stash_foreach(|_, message, _| {
                if let Some(rest) = message.split(STASH_MARKER).nth(1) {
                    if let Some((from, _)) = rest.split_once(" to ") {
                        set.insert(from.to_string());
                    }
                }
                true
            });
            set
        }

        /// Newest githist stash created when leaving `branch`, if any.
        pub fn find_githist_stash(&mut self, branch: &str) -> Option<usize> {
            let needle = format!("{STASH_MARKER}{branch} to ");
            let mut found = None;
            let _ = self.inner.stash_foreach(|index, message, _| {
                if found.is_none() && message.contains(&needle) {
                    found = Some(index);
                    return false;
                }
                true
            });
            found
        }

        /// # Errors
        ///
        /// Will return `git2::Error` if the stash could not be applied cleanly.
        pub fn pop_stash(&mut self, index: usize) -> Result<(), git2::Error> {
            self.inner.stash_pop(index, None)
        }

        fn worktree_holding_branch(
            &self,
            branch_name: &str,
        ) -> Result<Option<String>, git2::Error> {
            Ok(self.other_worktree_paths()?.get(branch_name).cloned())
        }

        /// Map local branch name → absolute path of the worktree where it is checked out,
        /// excluding the current worktree.
        fn other_worktree_paths(&self) -> Result<HashMap<String, String>, git2::Error> {
            let mut map = HashMap::new();
            let current_workdir = self.inner.workdir().map(normalize_workdir);

            let mut record = |repo: &Repository, path: &Path| {
                let normalized = normalize_workdir(path);
                if current_workdir.as_ref() == Some(&normalized) {
                    return;
                }
                if let Some(name) = head_branch_of(repo) {
                    map.insert(name, normalized.display().to_string());
                }
            };

            // Main working tree (git2's worktrees() list omits it).
            if let Ok(main_repo) = Repository::open(self.inner.commondir()) {
                if let Some(wd) = main_repo.workdir() {
                    record(&main_repo, wd);
                }
            }

            let worktrees = self.inner.worktrees()?;
            for i in 0..worktrees.len() {
                let Some(wt_name) = worktrees.get(i)? else {
                    continue;
                };
                let wt = self.inner.find_worktree(wt_name)?;
                let wt_path = wt.path().to_path_buf();
                let wt_repo = match Repository::open(&wt_path) {
                    Ok(repo) => repo,
                    Err(_) => match Repository::open_from_worktree(&wt) {
                        Ok(repo) => repo,
                        Err(_) => continue,
                    },
                };
                record(&wt_repo, &wt_path);
            }

            Ok(map)
        }

        /// # Errors
        ///
        /// Will return `git2::Error` if branch change failed.
        pub fn change_branch(&self, branch_name: &str) -> Result<(), git2::Error> {
            if let Some(path) = self.worktree_holding_branch(branch_name)? {
                return Err(git2::Error::from_str(&format!(
                    "branch '{branch_name}' is already checked out in {path}"
                )));
            }

            let refname = format!("refs/heads/{branch_name}");
            let target = self.inner.revparse_single(&refname)?;
            // Safe checkout: carries compatible local changes, errors on conflict
            // BEFORE HEAD moves, so a failure leaves the repo untouched.
            self.inner.checkout_tree(&target, None)?;
            self.inner.set_head(&refname)?;
            Ok(())
        }

        /// True when `branch_name`'s tip is reachable from HEAD.
        ///
        /// # Errors
        ///
        /// Will return `git2::Error` if the branch or HEAD can't be resolved.
        pub fn is_merged_into_head(&self, branch_name: &str) -> Result<bool, git2::Error> {
            let branch = self.inner.find_branch(branch_name, BranchType::Local)?;
            let branch_oid = branch
                .get()
                .target()
                .ok_or_else(|| git2::Error::from_str("branch has no target"))?;
            let head_oid = self
                .inner
                .head()?
                .target()
                .ok_or_else(|| git2::Error::from_str("HEAD has no target"))?;
            if branch_oid == head_oid {
                return Ok(true);
            }
            self.inner.graph_descendant_of(head_oid, branch_oid)
        }

        /// Create a branch at HEAD and switch to it.
        ///
        /// # Errors
        ///
        /// Will return `git2::Error` if creation or checkout failed.
        pub fn create_branch(&self, name: &str) -> Result<(), git2::Error> {
            let head = self.inner.head()?.peel_to_commit()?;
            self.inner.branch(name, &head, false)?;
            self.change_branch(name)
        }

        /// Create a local tracking branch for `remote_name` (e.g. "origin/feature")
        /// and switch to it. Returns the local branch name.
        ///
        /// # Errors
        ///
        /// Will return `git2::Error` if creation, upstream setup, or checkout failed.
        pub fn checkout_remote(&self, remote_name: &str) -> Result<String, git2::Error> {
            let local_name = local_branch_name(remote_name).to_string();
            let remote_branch = self.inner.find_branch(remote_name, BranchType::Remote)?;
            let commit = remote_branch.get().peel_to_commit()?;
            let mut local = self.inner.branch(&local_name, &commit, false)?;
            local.set_upstream(Some(remote_name))?;
            self.change_branch(&local_name)?;
            Ok(local_name)
        }

        /// # Errors
        ///
        /// Will return `git2::Error` if branch deletion failed.
        pub fn delete_branch(&self, branch_name: &str) -> Result<(), git2::Error> {
            let mut branch = self.inner.find_branch(branch_name, BranchType::Local)?;
            branch.delete()
        }
    }
}
