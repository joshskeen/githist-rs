# githist Improvements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the stash lifecycle, sort by checkout recency, add fuzzy filtering, support remote/new branches, fix UI correctness, and establish a test foundation for githist.

**Architecture:** All git2 usage stays behind `Repo` in `src/git.rs`. App state moves to a `Mode` enum (Normal/Filter/ConfirmDelete/DirtyPrompt/ConfirmCreate) in `src/lib.rs` with index-based filtering (`Vec<FilterEntry>`). Rendering stays in `src/ui.rs`; the event loop in `src/ui/run.rs` dispatches per mode and returns an optional farewell message printed after the TUI exits.

**Tech Stack:** Rust 2021, ratatui 0.30, crossterm 0.29, git2 0.21, tempfile (dev-only). The `pad` dependency is removed.

**Spec:** `docs/superpowers/specs/2026-06-12-githist-improvements-design.md`

**Deviation from spec (noted):** checkout recency uses a reflog *rank* (position from oldest, higher = more recent) instead of a timestamp, because reflog entries created in the same second would tie and make ordering nondeterministic. Field: `checkout_rank: Option<i64>`.

---

### Task 1: Test infrastructure

**Files:**
- Modify: `Cargo.toml`
- Create: `tests/common/mod.rs`
- Create: `tests/git_test.rs`

- [ ] **Step 1: Add tempfile dev-dependency**

Append to `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create test helpers**

Create `tests/common/mod.rs`:

```rust
use git2::{Repository, Signature, Time};
use githist::git::branching::{Config, Repo};
use std::fs;
use std::path::Path;

pub fn sig() -> Signature<'static> {
    Signature::now("Test", "test@example.com").unwrap()
}

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

pub fn open_githist_repo(dir: &Path) -> Repo {
    let config = Config {
        repo_path: dir.to_string_lossy().to_string(),
        tick_rate_ms: 250,
    };
    Repo::open(&config).unwrap()
}
```

- [ ] **Step 3: Write first integration test (current behavior)**

Create `tests/git_test.rs`:

```rust
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
```

Note: `get_branch_names` takes `&self` today; the test calls it on `mut` to be forward-compatible with Task 6 (where it becomes `&mut self`). The unused-mut warning until then is acceptable.

- [ ] **Step 4: Run test, verify pass**

Run: `cargo test --test git_test`
Expected: PASS (1 test)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock tests/
git commit -m "test: add integration test infrastructure with tempfile repos"
```

---

### Task 2: Fuzzy matching module

**Files:**
- Create: `src/fuzzy.rs`
- Modify: `src/lib.rs` (add `pub mod fuzzy;`)

- [ ] **Step 1: Write failing tests**

Create `src/fuzzy.rs`:

```rust
//! Case-insensitive subsequence fuzzy matching with simple contiguity scoring.

/// Match `needle` as a subsequence of `haystack`, case-insensitively (ASCII).
/// Returns `(score, matched char positions)` or `None` if it doesn't match.
/// Higher score = better match. Contiguous runs and early matches score higher.
#[must_use]
pub fn fuzzy_match(needle: &str, haystack: &str) -> Option<(i32, Vec<usize>)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::fuzzy_match;

    #[test]
    fn empty_needle_matches_everything() {
        let (score, positions) = fuzzy_match("", "feature-x").unwrap();
        assert_eq!(score, 0);
        assert!(positions.is_empty());
    }

    #[test]
    fn contiguous_substring_matches() {
        let (_, positions) = fuzzy_match("feat", "feature-x").unwrap();
        assert_eq!(positions, vec![0, 1, 2, 3]);
    }

    #[test]
    fn subsequence_matches_with_gaps() {
        let (_, positions) = fuzzy_match("fx", "feature-x").unwrap();
        assert_eq!(positions, vec![0, 8]);
    }

    #[test]
    fn non_matching_needle_returns_none() {
        assert!(fuzzy_match("z", "feature").is_none());
        assert!(fuzzy_match("featurex-", "feature-x").is_none());
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(fuzzy_match("FEAT", "feature").is_some());
        assert!(fuzzy_match("feat", "FEATURE").is_some());
    }

    #[test]
    fn contiguous_match_scores_higher_than_scattered() {
        let (contiguous, _) = fuzzy_match("fix", "fix-login").unwrap();
        let (scattered, _) = fuzzy_match("fix", "feature-import-x").unwrap();
        assert!(contiguous > scattered);
    }

    #[test]
    fn earlier_match_scores_higher() {
        let (early, _) = fuzzy_match("api", "api-v2").unwrap();
        let (late, _) = fuzzy_match("api", "legacy-api").unwrap();
        assert!(early > late);
    }
}
```

Add to `src/lib.rs` after `pub mod git;`:

```rust
pub mod fuzzy;
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test fuzzy`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement fuzzy_match**

Replace the `todo!()` body:

```rust
    if needle.is_empty() {
        return Some((0, Vec::new()));
    }
    let hay: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    let mut positions = Vec::with_capacity(needle_chars.len());
    let mut hi = 0;
    for nc in &needle_chars {
        let mut found = false;
        while hi < hay.len() {
            if hay[hi].eq_ignore_ascii_case(nc) {
                positions.push(hi);
                hi += 1;
                found = true;
                break;
            }
            hi += 1;
        }
        if !found {
            return None;
        }
    }
    let mut score = 0i32;
    for pair in positions.windows(2) {
        if pair[1] == pair[0] + 1 {
            score += 5;
        } else {
            score -= i32::try_from(pair[1] - pair[0]).unwrap_or(i32::MAX);
        }
    }
    score -= i32::try_from(positions[0]).unwrap_or(i32::MAX);
    Some((score, positions))
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test fuzzy`
Expected: PASS (7 tests)

- [ ] **Step 5: Commit**

```bash
git add src/fuzzy.rs src/lib.rs
git commit -m "feat: add case-insensitive subsequence fuzzy matcher"
```

---

### Task 3: Extend BranchInfo (summary, new fields, non-UTF8 safety)

**Files:**
- Modify: `src/git.rs`
- Modify: `tests/git_test.rs`

- [ ] **Step 1: Write failing test**

Append to `tests/git_test.rs`:

```rust
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
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test --test git_test branch_info_includes_commit_summary`
Expected: FAIL to compile (no `summary` field)

- [ ] **Step 3: Extend BranchInfo and builder**

In `src/git.rs`, replace the `BranchInfo` struct with:

```rust
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
```

Add a helper inside `mod branching` (module level, not in `impl Repo`):

```rust
    fn commit_fields(
        commit: &git2::Commit,
        formatter: &Formatter,
        now: DateTime<Utc>,
    ) -> (i64, String, String) {
        let last_commit_time = commit.time().seconds();
        let time_ago = DateTime::from_timestamp(last_commit_time, 0)
            .map_or_else(|| "unknown".to_string(), |dt| formatter.convert_chrono(dt, now));
        let summary = commit.summary().unwrap_or("").to_string();
        (last_commit_time, time_ago, summary)
    }
```

Replace the loop body of `get_branch_names` with (note: non-UTF8 names are now skipped instead of panicking; this is untestable portably, so no test covers it):

```rust
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

                result.push(BranchInfo {
                    branch_name,
                    last_commit_time,
                    time_ago,
                    summary,
                    is_head,
                    remote_tracking,
                    is_remote: false,
                    has_stash: false,
                    checkout_rank: None,
                });
            }
```

- [ ] **Step 4: Fix lib.rs compile (struct literal sites)**

No struct literals of `BranchInfo` exist outside `git.rs` yet; run `cargo build` and fix any missed sites.

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/git.rs tests/git_test.rs
git commit -m "feat: add commit summary and lifecycle fields to BranchInfo; skip non-UTF8 branch names"
```

---

### Task 4: Checkout-recency sorting and previous_branch

**Files:**
- Modify: `src/git.rs`
- Modify: `tests/git_test.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests/git_test.rs`:

```rust
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
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test --test git_test recency`
Expected: FAIL (no `previous_branch`; ordering wrong)

- [ ] **Step 3: Implement reflog parsing**

Add `use std::collections::HashMap;` to the imports in `src/git.rs`. Add to `impl Repo`:

```rust
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
                let Some(msg) = entry.message() else { continue };
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
                let Some(msg) = entry.message() else { continue };
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
```

(Reflog iteration starts at the newest entry, so `or_insert` keeps the most recent rank.)

- [ ] **Step 4: Wire rank into get_branch_names and change sorting**

In `get_branch_names`, before the loop add:

```rust
            let recency = self.checkout_recency();
```

In the `BranchInfo` literal change:

```rust
                    checkout_rank: recency.get(&branch_name).copied(),
```

(Place `let checkout_rank = recency.get(&branch_name).copied();` before the literal since `branch_name` moves into it, then use `checkout_rank`.)

Replace the sort lines (`result.sort_by_key(...); result.reverse();`) with:

```rust
            result.sort_by(|a, b| {
                let ka = (a.checkout_rank.unwrap_or(i64::MIN), a.last_commit_time);
                let kb = (b.checkout_rank.unwrap_or(i64::MIN), b.last_commit_time);
                kb.cmp(&ka)
            });
```

- [ ] **Step 5: Run tests, verify pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/git.rs tests/git_test.rs
git commit -m "feat: sort branches by checkout recency from reflog; add previous_branch"
```

---

### Task 5: Safe carry-along checkout (change_branch via checkout_tree)

**Files:**
- Modify: `src/git.rs`
- Modify: `tests/git_test.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests/git_test.rs`:

```rust
use std::fs;

#[test]
fn change_branch_carries_compatible_dirty_changes() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "feature-a");

    fs::write(dir.path().join("untracked.txt"), "wip").unwrap();
    let githist_repo = open_githist_repo(dir.path());
    githist_repo.change_branch("feature-a").unwrap();

    assert!(dir.path().join("untracked.txt").exists());
    assert_eq!(
        repo.head().unwrap().shorthand(),
        Some("feature-a")
    );
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
    assert_eq!(repo.head().unwrap().shorthand(), Some("main"));
    assert_eq!(
        fs::read_to_string(dir.path().join("README.md")).unwrap(),
        "local edit"
    );
}
```

- [ ] **Step 2: Run tests, verify conflict test fails**

Run: `cargo test --test git_test change_branch`
Expected: `change_branch_fails_without_moving_head_on_conflict` FAILS (current code moves HEAD before checkout, so HEAD ends up on `other`). The carry test should pass already.

- [ ] **Step 3: Reorder checkout before set_head**

In `src/git.rs`, replace the tail of `change_branch` (after the worktree check) with:

```rust
            let refname = format!("refs/heads/{branch_name}");
            let target = self.inner.revparse_single(&refname)?;
            // Safe checkout: carries compatible local changes, errors on conflict
            // BEFORE HEAD moves, so a failure leaves the repo untouched.
            self.inner.checkout_tree(&target, None)?;
            self.inner.set_head(&refname)?;
            Ok(())
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/git.rs tests/git_test.rs
git commit -m "fix: checkout target tree before moving HEAD so conflicts leave repo untouched"
```

---

### Task 6: Stash lifecycle (find, pop, indicator)

**Files:**
- Modify: `src/git.rs`
- Modify: `tests/git_test.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests/git_test.rs`:

```rust
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
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test --test git_test stash`
Expected: FAIL to compile (no `find_githist_stash`)

- [ ] **Step 3: Implement stash helpers**

Add `HashSet` to the `std::collections` import in `src/git.rs`. Add a module-level constant and helper:

```rust
    const STASH_MARKER: &str = "githist: stash before switching from ";
```

In `stash_changes`, replace the `message` line with:

```rust
            let message = format!("{STASH_MARKER}{current} to {target_branch}");
```

Add to `impl Repo`:

```rust
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
```

- [ ] **Step 4: Make get_branch_names take &mut self and flag stashes**

Change the signature to `pub fn get_branch_names(&mut self) -> Result<Vec<BranchInfo>, git2::Error>`. At the top add:

```rust
            let stashed = self.stashed_branches();
```

In the `BranchInfo` construction set (computed before the literal, like `checkout_rank`):

```rust
                    has_stash: stashed.contains(&branch_name),
```

`src/main.rs` and `src/ui/run.rs` already hold `&mut Repo`, so callers compile unchanged.

- [ ] **Step 5: Run tests, verify pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/git.rs tests/git_test.rs
git commit -m "feat: find and pop githist stashes; flag branches with pending stashes"
```

---

### Task 7: Merge detection, branch creation, remote branches

**Files:**
- Modify: `src/git.rs`
- Modify: `tests/git_test.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests/git_test.rs`:

```rust
#[test]
fn detects_whether_branch_is_merged_into_head() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    create_branch(&repo, "merged-branch"); // same commit as HEAD
    let mut githist_repo = open_githist_repo(dir.path());
    githist_repo.change_branch("merged-branch").unwrap();
    commit_file_at(&repo, "b.txt", "b", "unmerged work", 1_000_300);
    githist_repo.change_branch("main").unwrap();

    assert!(!githist_repo.is_merged_into_head("merged-branch").unwrap());
    // main's tip is an ancestor of merged-branch's tip, not vice versa;
    // create a branch at HEAD to assert the merged case:
    create_branch(&repo, "at-head");
    assert!(githist_repo.is_merged_into_head("at-head").unwrap());
}

#[test]
fn creates_and_switches_to_new_branch() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    let githist_repo = open_githist_repo(dir.path());
    githist_repo.create_branch("brand-new").unwrap();
    assert_eq!(repo.head().unwrap().shorthand(), Some("brand-new"));
}

#[test]
fn lists_remote_branches_without_local_counterpart() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    let oid = repo.head().unwrap().target().unwrap();
    repo.reference("refs/remotes/origin/main", oid, false, "test").unwrap();
    repo.reference("refs/remotes/origin/remote-only", oid, false, "test").unwrap();

    let mut githist_repo = open_githist_repo(dir.path());
    let branches = githist_repo.get_branch_names().unwrap();
    let names: Vec<&str> = branches.iter().map(|b| b.branch_name.as_str()).collect();
    assert!(names.contains(&"origin/remote-only"));
    assert!(!names.contains(&"origin/main")); // shadowed by local main
    let remote = branches.iter().find(|b| b.branch_name == "origin/remote-only").unwrap();
    assert!(remote.is_remote);
}

#[test]
fn checkout_remote_creates_tracking_branch() {
    let dir = tempfile::tempdir().unwrap();
    let repo = init_repo(dir.path());
    let oid = repo.head().unwrap().target().unwrap();
    repo.reference("refs/remotes/origin/remote-only", oid, false, "test").unwrap();

    let githist_repo = open_githist_repo(dir.path());
    let local = githist_repo.checkout_remote("origin/remote-only").unwrap();
    assert_eq!(local, "remote-only");
    assert_eq!(repo.head().unwrap().shorthand(), Some("remote-only"));
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test --test git_test`
Expected: new tests FAIL to compile

- [ ] **Step 3: Implement git operations**

Add module-level helper in `mod branching`:

```rust
    /// Local branch name for a remote branch like "origin/feature".
    #[must_use]
    pub fn local_branch_name(remote_name: &str) -> &str {
        remote_name.split_once('/').map_or(remote_name, |(_, suffix)| suffix)
    }
```

Add to `impl Repo`:

```rust
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
```

- [ ] **Step 4: List remote branches in get_branch_names**

In `get_branch_names`, collect local names while looping (`let mut local_names = HashSet::new();` before the loop, `local_names.insert(branch_name.clone());` first thing in the loop body). After the local-branch loop, add:

```rust
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
                });
            }
```

- [ ] **Step 5: Run tests, verify pass**

Run: `cargo test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/git.rs tests/git_test.rs
git commit -m "feat: merge detection, branch creation, and remote branch checkout"
```

---

### Task 8: App state refactor (Mode enum, index-based fuzzy filtering)

**Files:**
- Modify: `src/lib.rs` (full rewrite below)
- Modify: `src/ui.rs` (mechanical adaptation)
- Modify: `src/ui/run.rs` (mechanical adaptation)

This task lands the state model; visuals (Task 9) and behaviors (Task 10) follow. All three files change together so every commit compiles.

- [ ] **Step 1: Rewrite src/lib.rs**

Replace `src/lib.rs` entirely with:

```rust
use crate::git::branching::BranchInfo;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::ListState;
use ratatui::Terminal;
use std::io::Stdout;

pub mod fuzzy;
pub mod git;
pub mod ui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    ConfirmDelete { branch_name: String, merged: bool },
    DirtyPrompt { target: BranchInfo },
    ConfirmCreate { name: String },
}

/// One visible row: an index into `StatefulList::items` plus the char
/// positions in the branch name matched by the current filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterEntry {
    pub index: usize,
    pub positions: Vec<usize>,
}

pub struct StatefulList {
    pub state: ListState,
    pub items: Vec<BranchInfo>,
    pub filtered: Vec<FilterEntry>,
}

pub struct App {
    pub items: StatefulList,
    pub filter: String,
    pub mode: Mode,
    pub pending: String,
}

impl StatefulList {
    fn with_items(items: Vec<BranchInfo>) -> StatefulList {
        let filtered = (0..items.len())
            .map(|index| FilterEntry { index, positions: Vec::new() })
            .collect();
        StatefulList {
            state: ListState::default(),
            items,
            filtered,
        }
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }

    pub fn next(&mut self) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= len - 1 {
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
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn page_down(&mut self, page_size: usize) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        self.state.select(Some((i + page_size).min(len - 1)));
    }

    pub fn page_up(&mut self, page_size: usize) {
        let len = self.filtered.len();
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        self.state.select(Some(i.saturating_sub(page_size)));
    }

    pub fn go_to_first(&mut self) {
        if !self.filtered.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn go_to_last(&mut self) {
        let len = self.filtered.len();
        if len > 0 {
            self.state.select(Some(len - 1));
        }
    }
}

pub struct NoSelectionError;

impl App {
    #[must_use]
    pub fn new(branches: Vec<BranchInfo>) -> App {
        App {
            items: StatefulList::with_items(branches),
            filter: String::new(),
            mode: Mode::Normal,
            pending: String::new(),
        }
    }

    pub fn select_first_item_if_none(&mut self) {
        if self.items.state.selected().is_none() && !self.items.filtered.is_empty() {
            self.items.state.select(Some(0));
        }
    }

    /// # Errors
    ///
    /// Will return `NoSelectionError` if a branch was not selected.
    pub fn get_selected_branch_info(&self) -> Result<BranchInfo, NoSelectionError> {
        let index = self.items.state.selected().ok_or(NoSelectionError)?;
        let entry = self.items.filtered.get(index).ok_or(NoSelectionError)?;
        self.items.items.get(entry.index).cloned().ok_or(NoSelectionError)
    }

    /// # Errors
    ///
    /// Will return `NoSelectionError` if a branch was not selected.
    pub fn get_selected_branch_name(&self) -> Result<String, NoSelectionError> {
        self.get_selected_branch_info().map(|info| info.branch_name)
    }

    #[must_use]
    pub fn filtered_len(&self) -> usize {
        self.items.filtered.len()
    }

    #[must_use]
    pub fn total_len(&self) -> usize {
        self.items.items.len()
    }

    /// Set a status message and redraw immediately (used before long operations).
    pub fn show_status(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        status: String,
    ) {
        self.pending = status;
        terminal.draw(|f| self.ui(f)).expect("error updating!");
    }

    pub(crate) fn update_filtered(&mut self) {
        let mut scored: Vec<(i32, FilterEntry)> = self
            .items
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, branch)| {
                fuzzy::fuzzy_match(&self.filter, &branch.branch_name)
                    .map(|(score, positions)| (score, FilterEntry { index, positions }))
            })
            .collect();
        if !self.filter.is_empty() {
            scored.sort_by(|a, b| b.0.cmp(&a.0));
        }
        self.items.filtered = scored.into_iter().map(|(_, entry)| entry).collect();
        if self.items.filtered.is_empty() {
            self.items.state.select(None);
        } else {
            self.items.state.select(Some(0));
        }
    }

    pub fn set_branches(&mut self, branches: Vec<BranchInfo>) {
        self.items.items = branches;
        self.update_filtered();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(name: &str) -> BranchInfo {
        BranchInfo {
            branch_name: name.to_string(),
            last_commit_time: 0,
            time_ago: String::new(),
            summary: String::new(),
            is_head: false,
            remote_tracking: None,
            is_remote: false,
            has_stash: false,
            checkout_rank: None,
        }
    }

    fn app_with(names: &[&str]) -> App {
        App::new(names.iter().map(|n| branch(n)).collect())
    }

    #[test]
    fn empty_filter_keeps_original_order() {
        let mut app = app_with(&["b-one", "a-two", "c-three"]);
        app.update_filtered();
        let order: Vec<usize> = app.items.filtered.iter().map(|e| e.index).collect();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn filter_narrows_and_ranks_contiguous_first() {
        let mut app = app_with(&["release-fix", "fix-login", "docs"]);
        app.filter = "fix".to_string();
        app.update_filtered();
        let names: Vec<&str> = app
            .items
            .filtered
            .iter()
            .map(|e| app.items.items[e.index].branch_name.as_str())
            .collect();
        assert_eq!(names, vec!["fix-login", "release-fix"]);
    }

    #[test]
    fn no_match_clears_selection() {
        let mut app = app_with(&["main"]);
        app.filter = "zzz".to_string();
        app.update_filtered();
        assert_eq!(app.filtered_len(), 0);
        assert!(app.items.state.selected().is_none());
        assert!(app.get_selected_branch_info().is_err());
    }

    #[test]
    fn selection_maps_through_filtered_indices() {
        let mut app = app_with(&["alpha", "beta", "alpine"]);
        app.filter = "alp".to_string();
        app.update_filtered();
        app.items.state.select(Some(1));
        let name = app.get_selected_branch_name().map_err(|_| ()).unwrap();
        // "alpha" and "alpine" both match contiguously at position 0; "alpha"
        // (3 matched chars of 5) and "alpine" tie on score, original order kept.
        assert_eq!(name, "alpine");
    }
}
```

- [ ] **Step 2: Adapt src/ui.rs mechanically**

In `App::ui`:
- Replace the `items` construction's source `self.items.filtered.clone().unwrap_or_default().into_iter().map(|branch_info| {...})` with `self.items.filtered.iter().map(|entry| { let branch_info = &self.items.items[entry.index]; ... })` (clone fields where the old code consumed them: `branch_info.branch_name.clone()` etc. — Task 9 rewrites rendering anyway, keep changes minimal to compile).
- Replace the status text block with:

```rust
            let status_text = if !self.pending.is_empty() {
                format!("status: {}", self.pending)
            } else {
                match &self.mode {
                    crate::Mode::Filter => format!("filter: {}_", self.filter),
                    crate::Mode::Normal if !self.filter.is_empty() => format!(
                        "filter: {} (press / to edit, Backspace to clear)",
                        self.filter
                    ),
                    _ => String::new(),
                }
            };
```

- [ ] **Step 3: Adapt src/ui/run.rs mechanically**

Keep all current behavior, dispatching on the new state:
- Replace `if let Some(branch_name) = self.delete_confirmation.clone()` with `if let Mode::ConfirmDelete { branch_name, .. } = self.mode.clone()` and set `self.mode = Mode::Normal;` where `self.delete_confirmation = None;` was. Entering delete confirmation becomes `self.mode = Mode::ConfirmDelete { branch_name: info.branch_name.clone(), merged: true };` (real merge check arrives in Task 10).
- Replace `if self.filter_mode` with `if self.mode == Mode::Filter`, and `self.filter_mode = true/false` with `self.mode = Mode::Filter` / `self.mode = Mode::Normal`.
- Replace calls: `self.update_with_status_preserve_filter(terminal, status)` -> `self.show_status(terminal, status)`; `self.update_with_status(terminal, status)` -> `self.show_status(terminal, status)` (the filter-clearing variant is gone; switching exits the app on success so it doesn't matter); `self.clear_pending_status(terminal)` -> `self.show_status(terminal, String::new())`.
- Add `use crate::Mode;` to imports.

- [ ] **Step 4: Build, run all tests**

Run: `cargo build && cargo test`
Expected: builds clean; all unit + integration tests PASS

- [ ] **Step 5: Smoke test**

Run: `cargo run -- .` in the githist repo itself; verify list renders, `/`-filtering fuzzy-matches (e.g. typing `mn` matches `main`), Enter/q work.

- [ ] **Step 6: Commit**

```bash
git add src/lib.rs src/ui.rs src/ui/run.rs
git commit -m "refactor: Mode enum and index-based fuzzy filtering for app state"
```

---

### Task 9: UI overhaul (layout, colors, highlights, summary, indicators)

**Files:**
- Modify: `src/ui.rs` (full rewrite of the `gui` module body below)
- Modify: `Cargo.toml` (remove `pad`)

- [ ] **Step 1: Rewrite the ui() function and add the span helper**

Replace the `impl App` block (and the `use pad::PadStr;` import) in `src/ui.rs`'s `gui` module with:

```rust
    /// Render `name` as spans, bolding the fuzzy-matched `positions`, padded
    /// with trailing spaces to `pad_to` chars.
    fn highlighted_spans(
        name: &str,
        positions: &[usize],
        base: Style,
        highlight: Style,
        pad_to: usize,
    ) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut run = String::new();
        let mut run_highlighted = false;
        for (i, ch) in name.chars().enumerate() {
            let is_highlighted = positions.binary_search(&i).is_ok();
            if is_highlighted != run_highlighted && !run.is_empty() {
                let style = if run_highlighted { highlight } else { base };
                spans.push(Span::styled(std::mem::take(&mut run), style));
            }
            run_highlighted = is_highlighted;
            run.push(ch);
        }
        if !run.is_empty() {
            let style = if run_highlighted { highlight } else { base };
            spans.push(Span::styled(run, style));
        }
        let len = name.chars().count();
        if pad_to > len {
            spans.push(Span::raw(" ".repeat(pad_to - len)));
        }
        spans
    }

    impl App {
        pub(crate) fn ui(&mut self, f: &mut Frame) {
            let chunks = Layout::default()
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .direction(Direction::Vertical)
                .split(f.area());

            let largest_string_len = self
                .items
                .items
                .iter()
                .map(|x| x.branch_name.chars().count())
                .max()
                .unwrap_or(0);

            let items: Vec<ListItem> = self
                .items
                .filtered
                .iter()
                .map(|entry| {
                    let branch_info = &self.items.items[entry.index];
                    let base_style = if branch_info.is_remote {
                        Style::default().add_modifier(Modifier::DIM)
                    } else {
                        Style::default()
                    };
                    let match_style =
                        base_style.fg(Color::Magenta).add_modifier(Modifier::BOLD);

                    let mut spans = vec![Span::styled(
                        if branch_info.is_head { "* " } else { "  " },
                        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                    )];
                    spans.extend(highlighted_spans(
                        &branch_info.branch_name,
                        &entry.positions,
                        base_style,
                        match_style,
                        largest_string_len,
                    ));
                    spans.push(Span::styled(
                        format!("   {}", branch_info.time_ago),
                        base_style,
                    ));
                    let summary: String = branch_info.summary.chars().take(40).collect();
                    if !summary.is_empty() {
                        spans.push(Span::styled(
                            format!("   {summary}"),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                    if branch_info.has_stash {
                        spans.push(Span::styled(
                            "  \u{2691} stashed",
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    if let Some(remote) = branch_info.remote_tracking.as_deref() {
                        spans.push(Span::styled(
                            format!(" [{remote}]"),
                            Style::default().fg(Color::Cyan),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();

            let count_info = if self.filtered_len() == self.total_len() {
                format!("{} branches", self.total_len())
            } else {
                format!("{}/{} branches", self.filtered_len(), self.total_len())
            };

            let title = format!("choose recent branch  ({count_info})");
            let items = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .bg(Color::LightGreen)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            let instructions_text = "q/Esc: quit | j/k/\u{2193}/\u{2191}: navigate | \u{21a9}: switch | -: previous branch | Shift+D: delete | /: filter | g/G: first/last";
            let instructions_para = Paragraph::new(instructions_text)
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: true });

            f.render_stateful_widget(items, chunks[0], &mut self.items.state);
            f.render_widget(instructions_para, chunks[1]);

            let status_text = self.status_line();
            if !status_text.is_empty() {
                let status_para = Paragraph::new(status_text)
                    .block(Block::default().borders(Borders::NONE))
                    .wrap(Wrap { trim: true });
                f.render_widget(status_para, chunks[2]);
            }
        }

        fn status_line(&self) -> String {
            if !self.pending.is_empty() {
                return format!("status: {}", self.pending);
            }
            match &self.mode {
                Mode::Filter => {
                    if self.filtered_len() == 0 && !self.filter.is_empty() {
                        format!(
                            "filter: {}_  (no matches \u{2014} press Enter to create this branch)",
                            self.filter
                        )
                    } else {
                        format!("filter: {}_", self.filter)
                    }
                }
                Mode::ConfirmDelete { branch_name, merged } => {
                    if *merged {
                        format!("delete branch '{branch_name}'? [y/n]")
                    } else {
                        format!(
                            "delete branch '{branch_name}'? NOT merged into HEAD \u{2014} commits may be lost [y/n]"
                        )
                    }
                }
                Mode::DirtyPrompt { target } => format!(
                    "working tree is dirty \u{2014} switch to '{}': [s]tash changes / [b]ring along / [c]ancel",
                    target.branch_name
                ),
                Mode::ConfirmCreate { name } => {
                    format!("create branch '{name}' and switch to it? [y/n]")
                }
                Mode::Normal => {
                    if self.filter.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "filter: {} (press / to edit, Backspace to clear)",
                            self.filter
                        )
                    }
                }
            }
        }
    }
```

Update the module imports: remove `use pad::PadStr;`, add `use crate::Mode;`.

- [ ] **Step 2: Remove pad from Cargo.toml**

Delete the line `pad = "0.1.6"` from `[dependencies]`.

- [ ] **Step 3: Build and test**

Run: `cargo build && cargo test`
Expected: clean build, all tests PASS

- [ ] **Step 4: Smoke test small terminal**

Run `cargo run -- .` and resize the terminal to ~10 rows: list, help line, and status line must all stay visible. Verify rows use the terminal's default colors (no forced white background), fuzzy-matched chars highlight while filtering, and the current branch summary shows dimmed.

- [ ] **Step 5: Commit**

```bash
git add src/ui.rs Cargo.toml Cargo.lock
git commit -m "feat: theme-friendly rendering with match highlights, summaries, stash flags; fix small-terminal layout"
```

---

### Task 10: Event loop behaviors (switch flow, prompts, previous-branch, farewell)

**Files:**
- Modify: `src/ui/run.rs` (full rewrite below)
- Modify: `src/main.rs` (full rewrite below)

- [ ] **Step 1: Rewrite src/ui/run.rs**

```rust
pub mod app {
    use crate::git::branching::{local_branch_name, BranchInfo, Config, Repo};
    use crate::{App, Mode};
    use crossterm::event;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::CrosstermBackend;
    use ratatui::Terminal;
    use std::io;
    use std::io::Stdout;
    use std::time::{Duration, Instant};

    const PAGE_SIZE: usize = 10;

    type Term = Terminal<CrosstermBackend<Stdout>>;

    enum Outcome {
        Stay,
        Exit(Option<String>),
    }

    impl App {
        /// Runs the event loop. Returns a farewell message to print after the
        /// terminal is restored (`None` when the user just quit).
        ///
        /// # Errors
        ///
        /// Will return `Err` if drawing or event polling failed.
        pub fn run_app(
            &mut self,
            config: &Config,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> io::Result<Option<String>> {
            let mut last_tick = Instant::now();
            loop {
                terminal.draw(|f| self.ui(f))?;

                let timeout = config
                    .tick_rate()
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout)? {
                    if let Event::Key(key) = event::read()? {
                        let outcome = match self.mode.clone() {
                            Mode::ConfirmDelete { branch_name, .. } => {
                                self.handle_confirm_delete(key, &branch_name, repo);
                                Outcome::Stay
                            }
                            Mode::DirtyPrompt { target } => {
                                self.handle_dirty_prompt(key, &target, repo, terminal)
                            }
                            Mode::ConfirmCreate { name } => {
                                self.handle_confirm_create(key, &name, repo)
                            }
                            Mode::Filter => self.handle_filter_mode(key, repo, terminal),
                            Mode::Normal => self.handle_normal_mode(key, repo, terminal),
                        };
                        if let Outcome::Exit(message) = outcome {
                            return Ok(message);
                        }
                    }
                }
                if last_tick.elapsed() >= config.tick_rate() {
                    last_tick = Instant::now();
                }
            }
        }

        fn handle_confirm_delete(&mut self, key: KeyEvent, branch_name: &str, repo: &mut Repo) {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.mode = Mode::Normal;
                    let selected_index = self.items.state.selected();
                    match repo.delete_branch(branch_name) {
                        Ok(()) => match repo.get_branch_names() {
                            Ok(branches) => {
                                self.set_branches(branches);
                                if let Some(idx) = selected_index {
                                    let new_len = self.filtered_len();
                                    if new_len > 0 {
                                        self.items.state.select(Some(idx.min(new_len - 1)));
                                    }
                                }
                                self.pending = format!("deleted branch: {branch_name}");
                            }
                            Err(error) => {
                                self.pending = format!(
                                    "deleted branch but failed to refresh list: {error}"
                                );
                            }
                        },
                        Err(error) => {
                            self.pending =
                                format!("couldn't delete branch {branch_name}: {error}");
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Backspace => {
                    self.mode = Mode::Normal;
                    self.pending.clear();
                }
                _ => {}
            }
        }

        fn handle_dirty_prompt(
            &mut self,
            key: KeyEvent,
            target: &BranchInfo,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.mode = Mode::Normal;
                    self.show_status(
                        terminal,
                        format!("stashing changes, then switching to {}", target.branch_name),
                    );
                    if let Err(error) = repo.stash_changes(&target.branch_name) {
                        self.pending = format!("couldn't stash changes: {error}");
                        return Outcome::Stay;
                    }
                    self.perform_switch(target, repo)
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    self.mode = Mode::Normal;
                    self.perform_switch(target, repo)
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_confirm_create(&mut self, key: KeyEvent, name: &str, repo: &mut Repo) -> Outcome {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    match repo.create_branch(name) {
                        Ok(()) => Outcome::Exit(Some(format!(
                            "created and switched to branch '{name}'"
                        ))),
                        Err(error) => {
                            self.pending = format!("couldn't create branch '{name}': {error}");
                            Outcome::Stay
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_filter_mode(
            &mut self,
            key: KeyEvent,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Outcome::Stay
                }
                KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    self.try_switch_selected(repo, terminal)
                }
                KeyCode::Up => {
                    self.items.previous();
                    Outcome::Stay
                }
                KeyCode::Down => {
                    self.items.next();
                    Outcome::Stay
                }
                KeyCode::Backspace => {
                    if self.filter.pop().is_none() {
                        self.mode = Mode::Normal;
                    }
                    self.update_filtered();
                    Outcome::Stay
                }
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.filter.push(c);
                    self.update_filtered();
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_normal_mode(
            &mut self,
            key: KeyEvent,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            match key.code {
                KeyCode::Enter => self.try_switch_selected(repo, terminal),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Outcome::Exit(None),
                KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                    match self.get_selected_branch_info() {
                        Ok(info) => {
                            if info.is_head {
                                self.pending = format!(
                                    "can't delete '{}': it is the current branch",
                                    info.branch_name
                                );
                            } else if info.is_remote {
                                self.pending = format!(
                                    "'{}' is a remote branch; githist only deletes local branches",
                                    info.branch_name
                                );
                            } else {
                                let merged = repo
                                    .is_merged_into_head(&info.branch_name)
                                    .unwrap_or(false);
                                self.pending.clear();
                                self.mode = Mode::ConfirmDelete {
                                    branch_name: info.branch_name,
                                    merged,
                                };
                            }
                        }
                        Err(_) => {
                            self.pending = "no selection, nothing to delete!".to_string();
                        }
                    }
                    Outcome::Stay
                }
                KeyCode::Char('/') => {
                    self.mode = Mode::Filter;
                    self.pending.clear();
                    Outcome::Stay
                }
                KeyCode::Char('-') => match repo.previous_branch() {
                    Some(prev) => {
                        let info = self
                            .items
                            .items
                            .iter()
                            .find(|b| !b.is_remote && b.branch_name == prev)
                            .cloned();
                        match info {
                            Some(info) => self.request_switch(info, repo, terminal),
                            None => {
                                self.pending =
                                    format!("previous branch '{prev}' is not in the list");
                                Outcome::Stay
                            }
                        }
                    }
                    None => {
                        self.pending = "no previous branch in the reflog".to_string();
                        Outcome::Stay
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => {
                    self.items.next();
                    Outcome::Stay
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.items.previous();
                    Outcome::Stay
                }
                KeyCode::PageDown => {
                    self.items.page_down(PAGE_SIZE);
                    Outcome::Stay
                }
                KeyCode::PageUp => {
                    self.items.page_up(PAGE_SIZE);
                    Outcome::Stay
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    self.items.go_to_first();
                    Outcome::Stay
                }
                KeyCode::End | KeyCode::Char('G') => {
                    self.items.go_to_last();
                    Outcome::Stay
                }
                KeyCode::Left => {
                    self.items.unselect();
                    Outcome::Stay
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.update_filtered();
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        /// Switch to the selected branch; with no selection but a non-matching
        /// filter, offer to create a branch named after the filter text.
        fn try_switch_selected(&mut self, repo: &mut Repo, terminal: &mut Term) -> Outcome {
            match self.get_selected_branch_info() {
                Ok(info) => self.request_switch(info, repo, terminal),
                Err(_) => {
                    if !self.filter.is_empty() && self.filtered_len() == 0 {
                        self.mode = Mode::ConfirmCreate {
                            name: self.filter.clone(),
                        };
                    } else {
                        self.pending = "no selection, nothing to do!".to_string();
                    }
                    Outcome::Stay
                }
            }
        }

        /// Dirty trees prompt for stash/bring/cancel; clean trees switch directly.
        fn request_switch(
            &mut self,
            target: BranchInfo,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            if target.is_head {
                self.pending = format!("already on branch '{}'", target.branch_name);
                return Outcome::Stay;
            }
            match repo.is_dirty() {
                Ok(true) => {
                    self.mode = Mode::DirtyPrompt { target };
                    Outcome::Stay
                }
                Ok(false) => {
                    self.show_status(
                        terminal,
                        format!("switching to branch: {}", target.branch_name),
                    );
                    self.perform_switch(&target, repo)
                }
                Err(error) => {
                    self.pending = format!("couldn't check working tree status: {error}");
                    Outcome::Stay
                }
            }
        }

        /// Checkout (creating a tracking branch for remotes), then pop any
        /// githist stash that was taken when this branch was last left.
        fn perform_switch(&mut self, target: &BranchInfo, repo: &mut Repo) -> Outcome {
            let result = if target.is_remote {
                repo.checkout_remote(&target.branch_name).map(|local| {
                    format!(
                        "switched to new branch '{local}' tracking {}",
                        target.branch_name
                    )
                })
            } else {
                repo.change_branch(&target.branch_name)
                    .map(|()| format!("switched to branch '{}'", target.branch_name))
            };
            match result {
                Ok(mut message) => {
                    let stash_branch = if target.is_remote {
                        local_branch_name(&target.branch_name).to_string()
                    } else {
                        target.branch_name.clone()
                    };
                    if let Some(index) = repo.find_githist_stash(&stash_branch) {
                        match repo.pop_stash(index) {
                            Ok(()) => message.push_str("; restored stashed changes"),
                            Err(error) => message.push_str(&format!(
                                "; couldn't restore stashed changes ({error}) \u{2014} they remain in the stash list"
                            )),
                        }
                    }
                    Outcome::Exit(Some(message))
                }
                Err(error) => {
                    self.pending = format!("couldn't switch: {error}");
                    Outcome::Stay
                }
            }
        }
    }
}
```

- [ ] **Step 2: Rewrite src/main.rs**

```rust
use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use githist::git::branching::{Config, Repo};
use githist::ui::gui::{restore_terminal, setup_terminal};
use githist::App;
use std::io;
use std::panic;
use std::process::ExitCode;

fn main() -> ExitCode {
    let config = Config::parse();

    let mut repo = match Repo::open(&config) {
        Ok(repo) => repo,
        Err(error) => {
            eprintln!("couldn't open repository: {}", error.message());
            return ExitCode::FAILURE;
        }
    };

    let branches = match repo.get_branch_names() {
        Ok(branches) => branches,
        Err(error) => {
            eprintln!("couldn't read branches: {}", error.message());
            return ExitCode::FAILURE;
        }
    };

    let mut terminal = setup_terminal();

    // Install panic hook that restores the terminal before printing the panic.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let mut app = App::new(branches);
    app.select_first_item_if_none();
    let result = app.run_app(&config, &mut repo, &mut terminal);
    restore_terminal(&mut terminal).expect("couldn't restore terminal!");
    match result {
        Ok(Some(message)) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:?}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 3: Build, clippy, test**

Run: `cargo build && cargo clippy -- -D warnings && cargo test`
Expected: clean; all tests PASS. (If `show_status` callers were fully replaced, `update_with_status*`/`clear_pending_status` should no longer exist in lib.rs — verify with `grep -rn update_with_status src/`.)

- [ ] **Step 4: Smoke test all flows**

In a scratch repo (`/tmp/githist-smoke`): create branches, dirty the tree, run githist and verify: dirty prompt s/b/c; stash flag appears; switching back pops the stash and prints the farewell line; `-` toggles between last two branches; Enter on a remote branch creates a tracking branch; filtering to a non-existent name + Enter offers creation; Shift+D on an unmerged branch shows the warning.

- [ ] **Step 5: Commit**

```bash
git add src/ui/run.rs src/main.rs
git commit -m "feat: dirty-tree prompt, stash auto-restore, previous-branch key, branch creation, farewell message"
```

---

### Task 11: Docs and final verification

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Update README.md**

Replace lines 8-10 (`usage: ...` and `press Q to exit ...`) with:

```markdown
usage: `githist` followed by an optional path to a repo, defaulting to the working directory.

Branches are ordered by how recently you checked them out (from the reflog), then by last commit time. Remote-only branches are listed dimmed; selecting one creates a local tracking branch.

### keys

| key | action |
|-----|--------|
| `↓`/`↑` or `j`/`k` | choose branch |
| `↩` | switch to selected branch |
| `-` | switch to the previously checked-out branch |
| `/` | fuzzy filter (type to narrow, `↑`/`↓` to move, `↩` to switch, `Esc` to leave) |
| `Shift+D` | delete branch (warns if not merged into HEAD) |
| `g`/`G`, `Home`/`End`, `PgUp`/`PgDn` | jump around the list |
| `q`/`Esc` | quit |

If the working tree is dirty when switching, githist asks whether to stash the changes, bring them along, or cancel. Stashed changes are restored automatically the next time you switch back to the branch (look for the ⚑ stashed marker). Filtering to a name that matches nothing offers to create that branch.
```

- [ ] **Step 2: Update CHANGELOG.md**

Under `## [Unreleased]` add:

```markdown
### Added

- Restore githist stashes automatically when switching back to a branch; `⚑ stashed` marker on branches with pending stashes
- Prompt on dirty working tree: stash, bring changes along, or cancel (replaces unconditional auto-stash)
- Sort branches by checkout recency from the HEAD reflog, falling back to commit time
- `-` key to switch to the previously checked-out branch
- Fuzzy filtering with match highlighting; arrows navigate and Enter switches while filtering
- List remote-only branches (dimmed); selecting one creates a local tracking branch
- Offer to create a branch when the filter matches nothing
- Warn before deleting a branch that is not merged into HEAD
- Show the tip commit summary for each branch
- Print a summary message (switched/created/stash restored) on exit; exit code 1 on errors
- Integration and unit test suite

### Changed

- Checkout the target tree before moving HEAD so a conflicting switch leaves the repository untouched
- Use terminal default colors instead of forcing black-on-white rows
- Fix bottom help/status bars disappearing in short terminals
- Skip branches with non-UTF8 names instead of panicking

### Removed

- `pad` dependency
```

- [ ] **Step 3: Final verification**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && cargo build --release`
Expected: all clean and passing.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: document new keys, dirty-tree prompt, and stash lifecycle"
```
