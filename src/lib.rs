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

    /// # Panics
    ///
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.filtered.clone().unwrap().len() - 1 {
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
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

pub struct NoSelectionError;

impl App {
    #[must_use]
    pub fn new(branches: Vec<BranchInfo>) -> App {
        App {
            items: StatefulList::with_items(branches),
            filter: String::new(),
            pending: String::new(),
            delete_confirmation: None,
        }
    }
    pub fn select_first_item_if_none(&mut self) {
        match self.items.state.selected() {
            None => {
                // no selection. select the first.
                self.items.state.select(Some(0));
            }
            Some(_) => {}
        }
    }

    /// # Errors
    ///
    /// Will return `NoSelectionError` if a branch was not selected.
    /// # Panics
    ///
    pub fn get_selected_branch_name(&mut self) -> Result<String, NoSelectionError> {
        let option = self.items.state.selected();
        match option {
            None => Err(NoSelectionError),
            Some(index) => {
                let x = self.items.filtered.clone().unwrap().to_vec();
                Ok(x[index].branch_name.to_string())
            }
        }
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
                    x.branch_name.to_lowercase().contains(&self.filter)
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
