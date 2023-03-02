use std::error::Error;
use git2;
use chrono::{Datelike, Timelike, Utc};
use git2::{Branch, BranchType, Repository, Time};
use tui::widgets::ListState;

#[derive(Debug)]
pub struct BranchInfo {
    pub branch_name: String,
    pub last_commit_time: i64,
}

pub struct Config {
    pub repo_path: String,
}

pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

pub struct App {
    pub items: StatefulList<BranchInfo>,
}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }
    pub fn unselect(&mut self) {
        self.state.select(None);
    }
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

pub struct NoSelectionError;

impl<'a> App {
    pub fn new(branches: Vec<BranchInfo>) -> App {
        App {
            items: StatefulList::with_items(branches)
        }
    }
    pub fn get_selected_branch_name(self) -> Result<String, NoSelectionError> {
        let option = self.items.state.selected();
        match option {
            None => {
                Err(NoSelectionError)
            }
            Some(index) => {
                Ok(self.items.items[index].branch_name.to_string())
            }
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