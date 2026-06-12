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
        // "alpha" and "alpine" both match contiguously at position 0 and tie
        // on score, so the original order is kept; index 1 is "alpine".
        assert_eq!(name, "alpine");
    }
}
