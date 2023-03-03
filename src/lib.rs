use std::io;
use std::io::Stdout;
use std::time::{Duration, Instant};
use crossterm::event;
use crossterm::event::{Event, KeyCode};
use pad::PadStr;
use tui::{Frame, Terminal};
use tui::backend::{Backend, CrosstermBackend};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::{Spans};
use tui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use crate::git::branching::{BranchInfo, change_branch, Config};
use crate::ui::app_ui::restore_terminal;

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
    ///
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
                                    // deal with this if theres an error.
                                    match change_branch(config, &name) {
                                        Ok(_) => {}
                                        Err(error) => {
                                            // restore the terminal, then print the error if one occurred
                                            // while changing branch.
                                            restore_terminal(terminal).expect("couldn't restore!");
                                            eprintln!("couldn't change branch. reason: {error}");
                                        }
                                    }
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
                            self.items.state.select(Some(0));
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
            .constraints([Constraint::Percentage(96), Constraint::Percentage(2), Constraint::Percentage(2)].as_ref())
            .direction(Direction::Vertical)
            .split(f.size());
        let largest_string_len = self.items
            .items
            .iter()
            .map(|x| x.branch_name.len())
            .max().unwrap();

        let items: Vec<ListItem> = self
            .items
            .items
            .iter()
            .filter(|item| {
                if self.filter.is_empty() {
                    true
                } else {
                    item.branch_name.to_lowercase().contains(&self.filter.to_lowercase())
                }
            })
            .map(|branch_info| {
                let branch_and_padding = branch_info.branch_name.pad_to_width(largest_string_len);
                let lines = vec![
                    Spans::from(format!("{}   changed {}", branch_and_padding, branch_info.time_ago)),
                ];
                ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
            })
            .collect();

        let items = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("choose recent branch"))
            .highlight_style(
                Style::default()
                    .bg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        let block = Block::default().borders(Borders::NONE);
        let instructions_text = "Q to exit. ↓/↑ to choose branch, ↩ to change to selected branch. type to filter branches.";
        let instructions_para = Paragraph::new(instructions_text).block(block).wrap(Wrap { trim: true });

        // list of branches
        f.render_stateful_widget(items, chunks[0], &mut self.items.state);

        // instructions
        f.render_widget(instructions_para, chunks[1]);

        // status
        if !self.filter.is_empty() {
            let status = format!("filter: {}", self.filter);
            let block_2 = Block::default().borders(Borders::NONE);
            let status_para = Paragraph::new(status).block(block_2).wrap(Wrap { trim: true });
            f.render_widget(status_para, chunks[2]);
        }
    }
}