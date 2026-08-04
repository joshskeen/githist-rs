use git2::{BranchType, Repository, Signature, Time, WorktreeAddOptions};
use githist::git::branching::{Config, Repo};
use std::fs;
use std::path::Path;

pub fn sig_at(seconds: i64) -> Signature<'static> {
    Signature::new("Test", "test@example.com", &Time::new(seconds, 0)).unwrap()
}

/// Init a repo with deterministic branch name "main" and one initial commit.
pub fn init_repo(dir: &Path) -> Repository {
    let repo = Repository::init(dir).unwrap();
    repo.set_head("refs/heads/main").unwrap();
    {
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
    }
    commit_file_at(&repo, "README.md", "hello", "initial commit", 1_000_000);
    repo
}

pub fn commit_file_at(
    repo: &Repository,
    name: &str,
    content: &str,
    message: &str,
    seconds: i64,
) -> git2::Oid {
    let workdir = repo.workdir().unwrap();
    fs::write(workdir.join(name), content).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(name)).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let s = sig_at(seconds);
    repo.commit(Some("HEAD"), &s, &s, message, &tree, &parents)
        .unwrap()
}

pub fn create_branch(repo: &Repository, name: &str) {
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch(name, &head, false).unwrap();
}

pub fn add_worktree_for_branch(repo: &Repository, wt_name: &str, wt_path: &Path, branch: &str) {
    let branch = repo.find_branch(branch, BranchType::Local).unwrap();
    let reference = branch.into_reference();
    let mut opts = WorktreeAddOptions::new();
    opts.reference(Some(&reference));
    repo.worktree(wt_name, wt_path, Some(&opts)).unwrap();
}

pub fn open_githist_repo(dir: &Path) -> Repo {
    let config = Config {
        repo_path: dir.to_string_lossy().to_string(),
        tick_rate_ms: 250,
    };
    Repo::open(&config).unwrap()
}
