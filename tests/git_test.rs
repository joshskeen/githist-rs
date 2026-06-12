mod common;

use common::*;
use std::fs;

#[test]
fn lists_local_branches_with_head_marked() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "feature-a");

    let mut githist_repo = open_githist_repo(dir.path());
    let branches = githist_repo.get_branch_names().unwrap();

    let names: Vec<&str> = branches.iter().map(|b| b.branch_name.as_str()).collect();
    assert!(names.contains(&"main"));
    assert!(names.contains(&"feature-a"));
    let main = branches.iter().find(|b| b.branch_name == "main").unwrap();
    assert!(main.is_head);
    let feature = branches.iter().find(|b| b.branch_name == "feature-a").unwrap();
    assert!(!feature.is_head);
}

#[test]
fn branch_info_includes_commit_summary() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    commit_file_at(&repo, "a.txt", "a", "add the a file", 1_000_100);

    let mut githist_repo = open_githist_repo(dir.path());
    let branches = githist_repo.get_branch_names().unwrap();
    let main = branches.iter().find(|b| b.branch_name == "main").unwrap();
    assert_eq!(main.summary, "add the a file");
    assert!(!main.is_remote);
    assert!(!main.has_stash);
    assert!(main.checkout_rank.is_none());
}

#[test]
fn branches_sort_by_checkout_recency_then_commit_time() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "feature-a");
    create_branch(&repo, "feature-b");
    create_branch(&repo, "never-visited");

    let mut githist_repo = open_githist_repo(dir.path());
    githist_repo.change_branch("feature-a").unwrap();
    githist_repo.change_branch("feature-b").unwrap();
    githist_repo.change_branch("main").unwrap();

    let branches = githist_repo.get_branch_names().unwrap();
    let names: Vec<&str> = branches.iter().map(|b| b.branch_name.as_str()).collect();
    // main was checked out last, then feature-b, then feature-a; never-visited has
    // no checkout entry and falls back to commit-time ordering at the end.
    assert_eq!(names, vec!["main", "feature-b", "feature-a", "never-visited"]);
}

#[test]
fn previous_branch_comes_from_reflog() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "feature-a");

    let mut githist_repo = open_githist_repo(dir.path());
    githist_repo.change_branch("feature-a").unwrap();
    assert_eq!(githist_repo.previous_branch().as_deref(), Some("main"));

    githist_repo.change_branch("main").unwrap();
    assert_eq!(githist_repo.previous_branch().as_deref(), Some("feature-a"));
    drop(repo);
}

#[test]
fn change_branch_carries_compatible_dirty_changes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "feature-a");

    fs::write(dir.path().join("untracked.txt"), "wip").unwrap();
    let githist_repo = open_githist_repo(dir.path());
    githist_repo.change_branch("feature-a").unwrap();

    assert!(dir.path().join("untracked.txt").exists());
    assert_eq!(repo.head().unwrap().shorthand().ok(), Some("feature-a"));
}

#[test]
fn change_branch_fails_without_moving_head_on_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "other");
    let githist_repo = open_githist_repo(dir.path());
    githist_repo.change_branch("other").unwrap();
    commit_file_at(&repo, "README.md", "other version", "diverge", 1_000_200);
    githist_repo.change_branch("main").unwrap();

    // local edit conflicts with the tree on `other`
    fs::write(dir.path().join("README.md"), "local edit").unwrap();
    let err = githist_repo.change_branch("other");
    assert!(err.is_err());
    assert_eq!(repo.head().unwrap().shorthand().ok(), Some("main"));
    assert_eq!(
        fs::read_to_string(dir.path().join("README.md")).unwrap(),
        "local edit"
    );
}

#[test]
fn githist_stash_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "feature-a");
    let mut githist_repo = open_githist_repo(dir.path());

    fs::write(dir.path().join("wip.txt"), "wip").unwrap();
    assert!(githist_repo.is_dirty().unwrap());
    githist_repo.stash_changes("feature-a").unwrap();
    githist_repo.change_branch("feature-a").unwrap();
    assert!(!dir.path().join("wip.txt").exists());

    // the stash was created when leaving main, so it's found when returning
    assert!(githist_repo.find_githist_stash("feature-a").is_none());
    let index = githist_repo.find_githist_stash("main").unwrap();
    githist_repo.change_branch("main").unwrap();
    githist_repo.pop_stash(index).unwrap();
    assert_eq!(fs::read_to_string(dir.path().join("wip.txt")).unwrap(), "wip");
    assert!(githist_repo.find_githist_stash("main").is_none());
}

#[test]
fn branches_with_pending_githist_stash_are_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "feature-a");
    let mut githist_repo = open_githist_repo(dir.path());

    fs::write(dir.path().join("wip.txt"), "wip").unwrap();
    githist_repo.stash_changes("feature-a").unwrap();
    githist_repo.change_branch("feature-a").unwrap();

    let branches = githist_repo.get_branch_names().unwrap();
    let main = branches.iter().find(|b| b.branch_name == "main").unwrap();
    let feature = branches.iter().find(|b| b.branch_name == "feature-a").unwrap();
    assert!(main.has_stash);
    assert!(!feature.has_stash);
}
