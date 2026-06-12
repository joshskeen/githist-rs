# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/joshskeen/githist-rs/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/joshskeen/githist-rs/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/joshskeen/githist-rs/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/joshskeen/githist-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/joshskeen/githist-rs/compare/d2ecf1a...671f372
