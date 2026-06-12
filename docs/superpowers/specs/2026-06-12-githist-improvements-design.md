# githist Improvements — Design

Date: 2026-06-12
Status: Approved (full recommendation set)

## Goal

Improve githist's core workflow — quickly getting back to a recent branch — by
completing the stash lifecycle, sorting by true checkout recency, adding fuzzy
filtering, supporting remote/new branches, fixing UI correctness issues, and
establishing a test foundation.

## Features

### 1. Stash lifecycle completion

- When switching **to** a branch, look for the newest stash whose message matches
  `githist: stash before switching from <that branch> ...` and pop it after a
  successful checkout. Popping is automatic; a status line reports it.
- Branches with a pending githist stash show a `⚑ stashed` indicator in the list.
- Stash matching is by message prefix scan over `stash_foreach`.

### 2. Dirty-tree prompt instead of unconditional auto-stash

- On Enter with a dirty tree, prompt in the status area:
  `working tree is dirty: [s]tash / [b]ring along / [c]ancel`.
- **stash** = current behavior (stash including untracked, then switch).
- **bring along** = attempt checkout with `CheckoutBuilder::safe()` (git's default
  carry-along semantics); if it conflicts, report the error and stay.
- **cancel** = return to normal mode.

### 3. Checkout-recency sorting

- Parse the HEAD reflog for `checkout: moving from X to Y` entries to build a
  most-recently-used map (branch -> last checkout time).
- Sort: branches with reflog entries by recency desc, then remaining branches by
  last commit time desc (current behavior as fallback).
- Add `-` keybinding: switch directly to the previously checked-out branch.

### 4. Fuzzy filtering

- Replace substring matching with fuzzy matching + match highlighting. Prefer a
  small dependency-light approach: implement subsequence fuzzy scoring inline
  (no nucleo dependency needed at this scale) with case-insensitive matching,
  ranked by score; highlight matched chars in the list.
- Filter-mode UX: Up/Down arrows move the selection while typing; Enter switches
  to the selected branch immediately (instead of just leaving filter mode);
  Esc clears filter mode (keeps filter text as today).

### 5. Remote branches and branch creation

- List remote branches (deduped against local ones) in a dimmed style with an
  `origin/` prefix. Selecting one creates a local tracking branch and checks
  it out.
- When the filter matches nothing, Enter offers: `create branch '<filter>'? [y/n]`.
  On confirm, create from HEAD and switch to it.

### 6. UI correctness fixes

- Remove hardcoded `fg(Black).bg(White)` row style; use terminal default colors,
  style only the highlight and accents.
- Replace percentage layout with `Constraint::Min(1)` for the list and
  `Constraint::Length(1)` for the help and status bars so they survive small
  terminals.
- Delete confirmation warns when the branch is not merged into HEAD:
  `'<name>' is NOT merged — commits will be lost`.
- Each row shows the tip commit summary (truncated) after the time-ago text.

### 7. Code health

- Replace `Option<Box<Vec<BranchInfo>>>` with `Vec<usize>` of indices into
  `items`; eliminates per-keystroke and per-frame clones and the None/empty
  conflation.
- Compute ahead/behind lazily? No — keep eager but measure; it stays simple and
  startup cost is acceptable for typical repos. (YAGNI; revisit if slow.)
- `main` exits with code 1 on errors instead of printing and returning Ok.
- Non-UTF8 branch names are skipped with a status note instead of panicking.

### 8. Tests

- Integration-style tests using git2 against temp repos (tempfile dev-dependency):
  branch listing/sorting, recency ordering from reflog, stash save/pop matching,
  delete-unmerged detection, worktree-holding detection.
- Unit tests for fuzzy scoring and filtered-index state transitions
  (selection preservation on delete, empty-filter behavior).

## Architecture notes

- `git.rs` grows: reflog parsing, stash find/pop, remote branch listing, create
  branch, merged-status check. Keep all git2 usage behind `Repo`.
- App state gains a `Mode` enum (Normal, Filter, ConfirmDelete{name, merged},
  DirtyPrompt{target}, ConfirmCreate{name}) replacing the ad-hoc
  `filter_mode`/`delete_confirmation` fields — the event loop dispatches on it.
- UI rendering stays in `ui.rs`; event handling in `ui/run.rs` dispatches per mode.

## Error handling

- All git errors surface in the status bar; the app never exits on a failed
  operation, only on successful switch or quit.

## Out of scope

- Async/lazy ahead-behind computation, preview pane (commit summary inline
  instead), configuration file, theming options.
