pub mod branching {
    use chrono::{DateTime, Utc};
    use clap::Parser;
    use git2::{BranchType, Repository};
    use std::time::Duration;
    use timeago::Formatter;

    #[derive(Debug, Eq, PartialEq, Clone)]
    pub struct BranchInfo {
        pub branch_name: String,
        pub last_commit_time: i64,
        pub time_ago: String,
        pub is_head: bool,
        pub remote_tracking: Option<String>,
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
                head.shorthand().map(String::from)
            } else {
                None
            }
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
            let branches = self.inner.branches(Some(BranchType::Local))?;
            let formatter = Formatter::new();
            let now = Utc::now();

            for branch in branches {
                let (branch, _) = branch?;
                let branch_name = branch.name()?;
                let branch_name = branch_name.expect("no branch name!?").to_string();
                let last_commit = branch.get().peel_to_commit()?;
                let last_commit_time = last_commit.time().seconds();
                let datetime: DateTime<Utc> =
                    DateTime::from_timestamp(last_commit_time, 0)
                        .expect("invalid commit timestamp");
                let time_ago = formatter.convert_chrono(datetime, now);
                let is_head = head_name.as_deref() == Some(branch_name.as_str());
                let remote_tracking = self.remote_tracking_info(&branch_name);

                result.push(BranchInfo {
                    branch_name,
                    last_commit_time,
                    time_ago,
                    is_head,
                    remote_tracking,
                });
            }
            result.sort_by_key(|d| d.last_commit_time);
            result.reverse();
            Ok(result)
        }

        /// # Errors
        ///
        /// Will return `git2::Error` if branch change failed.
        pub fn change_branch(&self, branch_name: &str) -> Result<(), git2::Error> {
            let refname = format!("refs/heads/{branch_name}");
            let obj = self.inner.revparse_single(&refname)?;
            self.inner.checkout_tree(&obj, None)?;
            self.inner.set_head(&refname)?;
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
