pub mod branching {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use git2::{BranchType, Repository};
    use std::time::Duration;
    use timeago::Formatter;

    #[derive(Debug, Eq, PartialEq, Clone)]
    pub struct BranchInfo {
        pub branch_name: String,
        pub last_commit_time: i64,
        pub time_ago: String,
    }

    pub struct BranchChangeFailureException;

    pub struct Config {
        pub repo_path: String,
        pub tick_rate: Duration,
    }

    impl Config {
        #[must_use]
        pub fn new(args: &[String]) -> Config {
            let path = if let Some(path) = &args.get(1) {
                (*path).to_string()
            } else {
                String::from(".")
            };

            Config {
                tick_rate: Duration::from_millis(250),
                repo_path: (*path).to_string(),
            }
        }
    }

    /// # Errors
    ///
    /// Will return `git2::Error` if not a valid repo.
    ///
    /// # Panics
    /// if the naive date unwrap fails.
    pub fn get_branch_names(config: &Config) -> Result<Vec<BranchInfo>, git2::Error> {
        let mut result = Vec::new();
        let repo = Repository::open(&config.repo_path)?;
        let branches = repo.branches(Some(BranchType::Local))?;
        for branch in branches {
            let (branch, _) = branch?;
            let branch_name = branch.name()?;
            let branch_name = branch_name.expect("no branch name!?").to_string();
            let last_commit = branch.get().peel_to_commit()?;
            let last_commit_time = last_commit.time().seconds();
            let naive = NaiveDateTime::from_timestamp_opt(last_commit_time, 0).unwrap();
            let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc);
            let formatter = Formatter::new();
            let now = Utc::now();
            let time_ago = formatter.convert_chrono(datetime, now);

            result.push(BranchInfo {
                branch_name,
                last_commit_time,
                time_ago,
            });
        }
        result.sort_by_key(|d| d.last_commit_time);
        result.reverse();
        Ok(result)
    }

    /// # Errors
    ///
    /// Will return `git2::Error` if branch change failed.
    ///
    /// # Arguments
    ///
    /// * `config`: configuration for the ui
    /// * `branch_name`: branch name to change to
    ///
    /// returns: Result<(), Error>
    pub fn change_branch(config: &Config, branch_name: &str) -> Result<(), git2::Error> {
        let repo = Repository::open(&config.repo_path).expect("cant open repo");
        let obj = repo.revparse_single(&("refs/heads/".to_owned() + branch_name))?;
        repo.checkout_tree(&obj, None)?;
        repo.set_head(&("refs/heads/".to_owned() + branch_name))?;
        Ok(())
    }
}
