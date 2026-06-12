pub mod branching {
    use chrono::{DateTime, Utc};
    use clap::Parser;
    use git2::{BranchType, ErrorCode, Repository, StashFlags, Status};
    use std::collections::HashMap;
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
    }

    fn commit_fields(
        commit: &git2::Commit,
        formatter: &Formatter,
        now: DateTime<Utc>,
    ) -> (i64, String, String) {
        let last_commit_time = commit.time().seconds();
        let time_ago = DateTime::from_timestamp(last_commit_time, 0)
            .map_or_else(|| "unknown".to_string(), |dt| formatter.convert_chrono(dt, now));
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

    impl Repo {
        /// # Errors
        ///
        /// Will return `git2::Error` if not a valid repo.
        pub fn open(config: &Config) -> Result<Repo, git2::Error> {
            let inner = Repository::open(&config.repo_path)?;
            Ok(Repo { inner })
        }

        /// Returns the name of the current HEAD branch, or None if detached.
        fn head_branch_name(&self) -> Option<String> {
            let head = self.inner.head().ok()?;
            if head.is_branch() {
                head.shorthand().ok().map(String::from)
            } else {
                None
            }
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
                let Ok(Some(msg)) = entry.message() else { continue };
                let Some(rest) = msg.strip_prefix("checkout: moving from ") else {
                    continue;
                };
                let Some((_, to)) = rest.split_once(" to ") else { continue };
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
                let Ok(Some(msg)) = entry.message() else { continue };
                let Some(rest) = msg.strip_prefix("checkout: moving from ") else {
                    continue;
                };
                let Some((from, _)) = rest.split_once(" to ") else { continue };
                if Some(from) != current.as_deref()
                    && self.inner.find_branch(from, BranchType::Local).is_ok()
                {
                    return Some(from.to_string());
                }
            }
            None
        }

        /// Compute ahead/behind info relative to the remote tracking branch.
        fn remote_tracking_info(
            &self,
            branch_name: &str,
        ) -> Option<String> {
            let branch = self
                .inner
                .find_branch(branch_name, BranchType::Local)
                .ok()?;
            let upstream = branch.upstream().ok()?;
            let local_oid = branch.get().target()?;
            let upstream_oid = upstream.get().target()?;
            let (ahead, behind) = self.inner.graph_ahead_behind(local_oid, upstream_oid).ok()?;
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
        pub fn get_branch_names(&self) -> Result<Vec<BranchInfo>, git2::Error> {
            let mut result = Vec::new();
            let head_name = self.head_branch_name();
            let recency = self.checkout_recency();
            let branches = self.inner.branches(Some(BranchType::Local))?;
            let formatter = Formatter::new();
            let now = Utc::now();

            for branch in branches {
                let (branch, _) = branch?;
                let Some(branch_name) = branch.name()?.map(String::from) else {
                    continue; // skip branches with non-UTF8 names
                };
                let last_commit = branch.get().peel_to_commit()?;
                let (last_commit_time, time_ago, summary) =
                    commit_fields(&last_commit, &formatter, now);
                let is_head = head_name.as_deref() == Some(branch_name.as_str());
                let remote_tracking = self.remote_tracking_info(&branch_name);
                let checkout_rank = recency.get(&branch_name).copied();

                result.push(BranchInfo {
                    branch_name,
                    last_commit_time,
                    time_ago,
                    summary,
                    is_head,
                    remote_tracking,
                    is_remote: false,
                    has_stash: false,
                    checkout_rank,
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
            let message = format!(
                "githist: stash before switching from {current} to {target_branch}"
            );
            let signature = self.inner.signature()?;
            match self.inner.stash_save(
                &signature,
                &message,
                Some(StashFlags::INCLUDE_UNTRACKED),
            ) {
                Ok(_) => Ok(()),
                Err(error) if error.code() == ErrorCode::NotFound => Ok(()),
                Err(error) => Err(error),
            }
        }

        fn worktree_holding_branch(
            &self,
            branch_name: &str,
        ) -> Result<Option<String>, git2::Error> {
            let refname = format!("refs/heads/{branch_name}");
            let current_workdir = self.inner.workdir().map(std::path::PathBuf::from);
            let worktrees = self.inner.worktrees()?;

            for i in 0..worktrees.len() {
                let Some(wt_name) = worktrees.get(i)? else {
                    continue;
                };
                let wt = self.inner.find_worktree(wt_name)?;
                let wt_path = wt.path();
                if current_workdir.as_deref() == Some(wt_path) {
                    continue;
                }
                let wt_repo = Repository::open_from_worktree(&wt)?;
                let head = wt_repo.head()?;
                if head.name()? == refname {
                    return Ok(Some(wt_path.display().to_string()));
                }
            }

            Ok(None)
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
            self.inner.set_head(&refname)?;
            self.inner.checkout_head(None)?;
            Ok(())
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
