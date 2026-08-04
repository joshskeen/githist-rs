# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0] - 2026-08-04

### Added

- Annotate branches checked out in another worktree with a magenta `W` gutter marker and truncated path; status bar shows the full path when selected
- Press Enter on a worktree-held branch to exit and print its absolute path on stdout (no checkout), for `cd "$(githist)"` wrappers
- Render the TUI on `/dev/tty` so stdout stays clean for worktree path emission
- Block deleting a branch that is checked out in another worktree
- Include the main worktree when detecting worktree-held branches (git2's `worktrees()` omits it)

## [0.6.0] - 2026-06-12

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

### Fixed

- Detect untracked-only changes when checking whether the tree is dirty (stash-on-switch previously missed them)

### Removed

- `pad` dependency

## [0.5.0] - 2026-06-12

### Added

- Stash local changes (including untracked files) before switching branches when the working tree is dirty
- Detect branches already checked out in another git worktree and show a clear error with the worktree path

### Changed

- Update dependencies: ratatui 0.30, crossterm 0.29, git2 0.21, timeago 0.6, clap 4.6, chrono 0.4.45
- Switch branches with `set_head` followed by `checkout_head` to avoid leaving a half-updated working tree on failure

## [0.4.0] - 2026-02-24

### Fixed

- Fix panics, wrong list indexing, redundant terminal setup, and case-sensitive filter matching ([#3](https://github.com/joshskeen/githist-rs/pull/3))

## [0.3.0] - 2026-02-24

### Added

- CLI via clap with `--help` and `--version`
- Reuse a single `Repository` handle instead of reopening on every operation
- Mark the current branch with a yellow `*` indicator
- Show remote tracking ahead/behind info in cyan
- Panic hook to restore the terminal on crash
- Stay in the app after a failed branch switch (show error in status bar)
- Prevent deleting the currently checked-out branch
- `Esc` and lowercase `q` to quit; `j`/`k` vim-style navigation
- `/` to enter filter mode; Page Up/Down, Home/End, `g`/`G` for navigation
- Branch count and filter match count in the title bar
- Preserve selection position after deleting a branch

## [0.2.0] - 2025-10-29

### Added

- Branch deletion with `Shift+D` confirmation ([#1](https://github.com/joshskeen/githist-rs/pull/1))

### Changed

- Migrate from tui-rs to ratatui
- Show branch switch status in the TUI before switching ([#2](https://github.com/joshskeen/githist-rs/pull/2))

[Unreleased]: https://github.com/joshskeen/githist-rs/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/joshskeen/githist-rs/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/joshskeen/githist-rs/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/joshskeen/githist-rs/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/joshskeen/githist-rs/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/joshskeen/githist-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/joshskeen/githist-rs/compare/d2ecf1a...671f372
