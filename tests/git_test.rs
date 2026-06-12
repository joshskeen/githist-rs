mod common;

use common::*;

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
