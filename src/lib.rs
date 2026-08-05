use crate::agent_store::{AgentStore, LinkedSession};
use crate::git::branching::BranchInfo;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::ListState;
use ratatui::Terminal;
use std::fs::File;
use std::path::PathBuf;

pub mod acp_sessions;
pub mod agent_store;
pub mod fuzzy;
pub mod git;
pub mod path_display;
pub mod ui;

pub type TuiTerminal = Terminal<CrosstermBackend<File>>;

/// How the TUI session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppExit {
    /// User quit without switching.
    Quit,
    /// Checked out or created a branch; message printed after the TUI closes.
    Farewell(String),
    /// Selected a branch held in another worktree; path printed alone on stdout.
    WorktreePath(String),
    /// Resume a linked agent session after switching branches.
    ResumeAgent {
        session_id: String,
        cwd: PathBuf,
    },
}

/// What to do when the user skips the resume picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostNav {
    Farewell(String),
    WorktreePath(String),
}

/// Whether branch navigation should offer a resume picker afterward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchIntent {
    Enter,
    ThenResume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    ConfirmDelete { branch_name: String, merged: bool },
    DirtyPrompt { target: BranchInfo },
    ConfirmCreate { name: String },
    LinkAgent {
        branch_name: String,
        candidates: Vec<LinkedSession>,
        selected: usize,
        paste_buffer: String,
    },
    ResumeAgent {
        branch_name: String,
        sessions: Vec<LinkedSession>,
        selected: usize,
        post_nav: PostNav,
    },
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
    pub agent_store: AgentStore,
    pub repo_id: String,
    pub repo_cwd: PathBuf,
    /// Set when the user presses `a` to switch then resume; cleared on cancel or picker entry.
    pub after_switch_resume: bool,
}

impl StatefulList {
    fn with_items(items: Vec<BranchInfo>) -> StatefulList {
        let filtered = (0..items.len())
            .map(|index| FilterEntry {
                index,
                positions: Vec::new(),
            })
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
    pub fn new(
        branches: Vec<BranchInfo>,
        agent_store: AgentStore,
        repo_id: String,
        repo_cwd: PathBuf,
    ) -> App {
        App {
            items: StatefulList::with_items(branches),
            filter: String::new(),
            mode: Mode::Normal,
            pending: String::new(),
            agent_store,
            repo_id,
            repo_cwd,
            after_switch_resume: false,
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
        self.items
            .items
            .get(entry.index)
            .cloned()
            .ok_or(NoSelectionError)
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
    pub fn show_status(&mut self, terminal: &mut TuiTerminal, status: String) {
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
            scored.sort_by_key(|b| std::cmp::Reverse(b.0));
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

/// Resolve the exit when the user skips the resume picker.
#[must_use]
pub fn skip_resume_exit(post_nav: PostNav) -> AppExit {
    match post_nav {
        PostNav::Farewell(message) => AppExit::Farewell(message),
        PostNav::WorktreePath(path) => AppExit::WorktreePath(path),
    }
}

/// Working directory for `agent --resume` after a branch switch.
#[must_use]
pub fn resume_cwd(post_nav: &PostNav, repo_cwd: &std::path::Path) -> PathBuf {
    match post_nav {
        PostNav::WorktreePath(path) => PathBuf::from(path),
        PostNav::Farewell(_) => repo_cwd.to_path_buf(),
    }
}

/// Whether navigation should enter the resume picker instead of exiting.
#[must_use]
pub fn should_enter_resume_picker(intent: SwitchIntent, after_switch_resume: bool) -> bool {
    matches!(intent, SwitchIntent::ThenResume) || after_switch_resume
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
            worktree_path: None,
        }
    }

    fn app_with(names: &[&str]) -> App {
        App::new(
            names.iter().map(|n| branch(n)).collect(),
            AgentStore::default(),
            "test-repo".to_string(),
            PathBuf::from("/tmp/test-repo"),
        )
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
        // "alpha" and "alpine" both match contiguously at position 0 and tie
        // on score, so the original order is kept; index 1 is "alpine".
        assert_eq!(name, "alpine");
    }

    #[test]
    fn should_enter_resume_picker_for_then_resume_intent() {
        assert!(should_enter_resume_picker(SwitchIntent::ThenResume, false));
        assert!(!should_enter_resume_picker(SwitchIntent::Enter, false));
        assert!(should_enter_resume_picker(SwitchIntent::Enter, true));
    }

    #[test]
    fn skip_resume_exit_maps_post_nav() {
        assert_eq!(
            skip_resume_exit(PostNav::Farewell("hi".to_string())),
            AppExit::Farewell("hi".to_string())
        );
        assert_eq!(
            skip_resume_exit(PostNav::WorktreePath("/wt".to_string())),
            AppExit::WorktreePath("/wt".to_string())
        );
    }

    #[test]
    fn resume_cwd_uses_worktree_or_repo() {
        let repo = PathBuf::from("/repo");
        assert_eq!(
            resume_cwd(&PostNav::WorktreePath("/wt".to_string()), &repo),
            PathBuf::from("/wt")
        );
        assert_eq!(
            resume_cwd(&PostNav::Farewell("ok".to_string()), &repo),
            repo
        );
    }
}
