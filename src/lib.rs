use std::io;
use std::io::Stdout;
use std::time::{Duration, Instant};
use crossterm::event;
use crossterm::event::{Event, KeyCode};
use tui::{Frame, Terminal};
use tui::backend::{Backend, CrosstermBackend};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::Spans;
use tui::widgets::{Block, Borders, List, ListItem, ListState};
use crate::git::branching::{BranchInfo, change_branch, Config};

pub mod git;
pub mod ui;

pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

pub struct App {
    pub items: StatefulList<BranchInfo>,
    pub filter: String,
}

impl App {}

impl<T> StatefulList<T> {
    fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }
    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
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
    pub fn get_selected_branch_name(&mut self) -> Result<String, NoSelectionError> {
        let option = self.items.state.selected();
        match option {
            None => {
                Err(NoSelectionError)
            }
            Some(index) => {
                Ok(self.items.items[index].branch_name.to_string())
            }
        }
    }

    /// # Errors
    ///
    /// Will return `Err` if `self.ui()` failed.
    pub fn run_app(
        &mut self,
        config: &Config,
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> io::Result<()> {
        let mut last_tick = Instant::now();
        loop {
            terminal.draw(|f| self.ui(f))?;

            let timeout = config.tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Enter => {
                            let branch_name = self.get_selected_branch_name();
                            match branch_name {
                                Ok(name) => {
                                    println!("change branch to {name}");
                                    change_branch(config, &name);
                                }
                                Err(_) => {
                                    println!("no selection, nothing to do!");
                                }
                            }
                            return Ok(());
                        }
                        KeyCode::Char('Q') => {
                            return Ok(());
                        }
                        KeyCode::Left => self.items.unselect(),
                        KeyCode::Down => self.items.next(),
                        KeyCode::Up => self.items.previous(),
                        KeyCode::Backspace => {
                            self.filter.pop();
                        }
                        // update the filter used to limit the vec of branches shown
                        KeyCode::Char(c) => {
                            self.filter.push(c);
                        }
                        _ => {}
                    }
                }
            }
            if last_tick.elapsed() >= config.tick_rate {
                last_tick = Instant::now();
            }
        }
    }

    fn ui<B: Backend>(&mut self, f: &mut Frame<B>) {
        let chunks = Layout::default()
            .constraints([Constraint::Percentage(100)].as_ref())
            .direction(Direction::Horizontal)
            .split(f.size());

        let items: Vec<ListItem> = self
            .items
            .items
            .iter()
            .filter(|item| {
                if self.filter.is_empty() {
                    true
                } else {
                    item.branch_name.contains(&self.filter)
                }
            })
            .map(|branch_info| {
                let lines = vec![Spans::from(branch_info.branch_name.to_string())];
                ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
            })
            .collect();

        self.select_first_item_if_none();

        let items = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("choose recent branch"))
            .highlight_style(
                Style::default()
                    .bg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(items, chunks[0], &mut self.items.state);
    }
}