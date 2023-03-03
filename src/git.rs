pub mod branching {
    use std::time::Duration;
    use git2::{BranchType, Repository};

    #[derive(Debug)]
    pub struct BranchInfo {
        pub branch_name: String,
        pub last_commit_time: i64,
    }

    pub struct Config {
        pub repo_path: String,
        pub tick_rate: Duration,
    }

    impl Config {
        pub fn new(args: Vec<String>) -> Config {
            let path = if let Some(path) = &args.get(1) {
                path.to_string()
            } else {
                String::from(".")
            };

            Config {
                tick_rate: Duration::from_millis(250),
                repo_path: path.to_string(),
            }
        }
    }

    pub fn get_branch_names(config: &Config) -> Result<Vec<BranchInfo>, git2::Error> {
        let mut result = Vec::new();
        let repo = Repository::open((*config).repo_path.to_string())?;
        let branches = repo.branches(Some(BranchType::Local))?;
        for branch in branches {
            let (branch, _) = branch?;
            let branch_name = branch.name()?;
            let branch_name = branch_name.expect("no branch name!?").to_string();
            let last_commit = branch.get().peel_to_commit()?;
            let last_commit_time = last_commit.time().seconds();
            result.push(BranchInfo { branch_name, last_commit_time })
        }
        result.sort_by_key(|d| d.last_commit_time);
        result.reverse();
        Ok(result)
    }

    pub fn change_branch(config: &Config, branch_name: &str) {
        let repo = Repository::open((*config).repo_path.to_string()).expect("cant open repo");
        let obj = repo.revparse_single(&("refs/heads/".to_owned() +
            branch_name)).unwrap();
        repo.checkout_tree(
            &obj,
            None,
        ).unwrap();
        repo.set_head(&("refs/heads/".to_owned() + branch_name)).unwrap();
    }
}