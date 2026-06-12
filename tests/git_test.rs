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
