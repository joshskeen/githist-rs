use crate::git::branching::BranchInfo;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::ListState;
use ratatui::Terminal;
use std::io::Stdout;

pub mod git;
pub mod ui;

pub struct StatefulList {
    pub state: ListState,
    pub items: Vec<BranchInfo>,
    pub filtered: Option<Box<Vec<BranchInfo>>>,
}

pub struct App {
    pub items: StatefulList,
    pub filter: String,
    pub filter_mode: bool,
    pub pending: String,
    pub delete_confirmation: Option<String>,
}

impl StatefulList {
    fn with_items(items: Vec<BranchInfo>) -> StatefulList {
        let filtered = Some(Box::new(items.clone()));
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
        let len = self
            .filtered
            .as_ref()
            .map_or(0, |f| f.len());
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
        let len = self
            .filtered
            .as_ref()
            .map_or(0, |f| f.len());
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
        let len = self.filtered.as_ref().map_or(0, |f| f.len());
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        let new_i = (i + page_size).min(len - 1);
        self.state.select(Some(new_i));
    }

    pub fn page_up(&mut self, page_size: usize) {
        let len = self.filtered.as_ref().map_or(0, |f| f.len());
        if len == 0 {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        let new_i = i.saturating_sub(page_size);
        self.state.select(Some(new_i));
    }

    pub fn go_to_first(&mut self) {
        let len = self.filtered.as_ref().map_or(0, |f| f.len());
        if len > 0 {
            self.state.select(Some(0));
        }
    }

    pub fn go_to_last(&mut self) {
        let len = self.filtered.as_ref().map_or(0, |f| f.len());
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
            filter_mode: false,
            pending: String::new(),
            delete_confirmation: None,
        }
    }
    pub fn select_first_item_if_none(&mut self) {
        if self.items.state.selected().is_none() {
            self.items.state.select(Some(0));
        }
    }

    /// # Errors
    ///
    /// Will return `NoSelectionError` if a branch was not selected.
    pub fn get_selected_branch_info(&self) -> Result<BranchInfo, NoSelectionError> {
        let index = self.items.state.selected().ok_or(NoSelectionError)?;
        let filtered = self.items.filtered.as_ref().ok_or(NoSelectionError)?;
        filtered.get(index).cloned().ok_or(NoSelectionError)
    }

    /// # Errors
    ///
    /// Will return `NoSelectionError` if a branch was not selected.
    pub fn get_selected_branch_name(&self) -> Result<String, NoSelectionError> {
        self.get_selected_branch_info()
            .map(|info| info.branch_name)
    }

    pub fn filtered_len(&self) -> usize {
        self.items.filtered.as_ref().map_or(0, |f| f.len())
    }

    pub fn total_len(&self) -> usize {
        self.items.items.len()
    }

    pub fn update_with_status(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        pending_status: String,
    ) {
        self.filter.clear();
        self.update_with_status_preserve_filter(terminal, pending_status);
    }

    pub fn update_with_status_preserve_filter(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        pending_status: String,
    ) {
        self.pending = pending_status;
        terminal.draw(|f| self.ui(f)).expect("error updating!");
    }

    pub fn clear_pending_status(&mut self, terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
        self.pending.clear();
        terminal.draw(|f| self.ui(f)).expect("error updating!");
    }

    fn update_filtered(&mut self) {
        let filtered: Vec<BranchInfo> = self
            .items
            .items
            .clone()
            .into_iter()
            .filter(|x| {
                if self.filter.is_empty() {
                    true
                } else {
                    x.branch_name.to_lowercase().contains(&self.filter.to_lowercase())
                }
            })
            .collect();
        self.items.filtered = if filtered.is_empty() {
            self.items.state.select(None);
            None
        } else {
            self.items.state.select(Some(0));
            Some(Box::new(filtered))
        };
    }

    pub fn set_branches(&mut self, branches: Vec<BranchInfo>) {
        self.items.items = branches;
        self.update_filtered();
    }
}
