pub mod app {
    use crate::git::branching::{Config, Repo};
    use crate::App;
    use crossterm::event;
    use crossterm::event::{Event, KeyCode, KeyModifiers};
    use ratatui::backend::CrosstermBackend;
    use ratatui::Terminal;
    use std::io;
    use std::io::Stdout;
    use std::time::{Duration, Instant};

    const PAGE_SIZE: usize = 10;

    impl App {
        /// # Errors
        ///
        /// Will return `Err` if `self.ui()` failed.
        ///
        pub fn run_app(
            &mut self,
            config: &Config,
            repo: &Repo,
            terminal: &mut Terminal<CrosstermBackend<Stdout>>,
        ) -> io::Result<()> {
            let mut last_tick = Instant::now();
            loop {
                terminal.draw(|f| self.ui(f))?;

                let timeout = config
                    .tick_rate()
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout)? {
                    if let Event::Key(key) = event::read()? {
                        // Delete confirmation mode
                        if let Some(branch_name) = self.delete_confirmation.clone() {
                            match key.code {
                                KeyCode::Char('Y') | KeyCode::Char('y') => {
                                    self.delete_confirmation = None;
                                    let selected_index = self.items.state.selected();
                                    match repo.delete_branch(&branch_name) {
                                        Ok(_) => match repo.get_branch_names() {
                                            Ok(branches) => {
                                                self.set_branches(branches);
                                                if let Some(idx) = selected_index {
                                                    let new_len = self.filtered_len();
                                                    if new_len > 0 {
                                                        let new_idx = idx.min(new_len - 1);
                                                        self.items.state.select(Some(new_idx));
                                                    }
                                                }
                                                let status =
                                                    format!("deleted branch: {}", branch_name);
                                                self.update_with_status_preserve_filter(
                                                    terminal, status,
                                                );
                                            }
                                            Err(error) => {
                                                let status = format!(
                                                    "deleted branch but failed to refresh list: {error}"
                                                );
                                                self.update_with_status_preserve_filter(
                                                    terminal, status,
                                                );
                                            }
                                        },
                                        Err(error) => {
                                            let status = format!(
                                                "couldn't delete branch {branch_name}: {error}"
                                            );
                                            self.update_with_status_preserve_filter(
                                                terminal, status,
                                            );
                                        }
                                    }
                                }
                                KeyCode::Char('N')
                                | KeyCode::Char('n')
                                | KeyCode::Esc
                                | KeyCode::Backspace => {
                                    self.delete_confirmation = None;
                                    self.clear_pending_status(terminal);
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // Filter mode: typing goes to the filter
                        if self.filter_mode {
                            match key.code {
                                KeyCode::Esc | KeyCode::Enter => {
                                    self.filter_mode = false;
                                }
                                KeyCode::Backspace => {
                                    if self.filter.pop().is_none() {
                                        self.filter_mode = false;
                                    }
                                    self.update_filtered();
                                }
                                KeyCode::Char(c)
                                    if key.modifiers.is_empty()
                                        || key.modifiers == KeyModifiers::SHIFT =>
                                {
                                    self.filter.push(c);
                                    self.update_filtered();
                                }
                                _ => {}
                            }
                            continue;
                        }

                        // Normal mode
                        match key.code {
                            KeyCode::Enter => {
                                match self.get_selected_branch_info() {
                                    Ok(info) => {
                                        if info.is_head {
                                            let status = format!(
                                                "already on branch '{}'",
                                                info.branch_name
                                            );
                                            self.update_with_status_preserve_filter(
                                                terminal, status,
                                            );
                                        } else {
                                            let status = format!(
                                                "switching to branch: {}",
                                                info.branch_name
                                            );
                                            self.update_with_status(terminal, status);
                                            match repo.change_branch(&info.branch_name) {
                                                Ok(_) => return Ok(()),
                                                Err(error) => {
                                                    let status = format!(
                                                        "couldn't change branch: {error}"
                                                    );
                                                    self.update_with_status_preserve_filter(
                                                        terminal, status,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        let status = "no selection, nothing to do!".to_string();
                                        self.update_with_status_preserve_filter(terminal, status);
                                    }
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                                return Ok(());
                            }
                            KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                                match self.get_selected_branch_info() {
                                    Ok(info) => {
                                        if info.is_head {
                                            let status = format!(
                                                "can't delete '{}': it is the current branch",
                                                info.branch_name
                                            );
                                            self.update_with_status_preserve_filter(
                                                terminal, status,
                                            );
                                        } else {
                                            self.delete_confirmation =
                                                Some(info.branch_name.clone());
                                            let status = format!(
                                                "confirm deleting branch {}? press Y to delete or N to cancel",
                                                info.branch_name
                                            );
                                            self.update_with_status_preserve_filter(
                                                terminal, status,
                                            );
                                        }
                                    }
                                    Err(_) => {
                                        let status = "no selection, nothing to delete!".to_string();
                                        self.update_with_status_preserve_filter(terminal, status);
                                    }
                                }
                            }
                            KeyCode::Char('/') => {
                                self.filter_mode = true;
                            }
                            KeyCode::Down | KeyCode::Char('j') => self.items.next(),
                            KeyCode::Up | KeyCode::Char('k') => self.items.previous(),
                            KeyCode::PageDown => self.items.page_down(PAGE_SIZE),
                            KeyCode::PageUp => self.items.page_up(PAGE_SIZE),
                            KeyCode::Home | KeyCode::Char('g') => self.items.go_to_first(),
                            KeyCode::End | KeyCode::Char('G') => self.items.go_to_last(),
                            KeyCode::Left => self.items.unselect(),
                            KeyCode::Backspace => {
                                self.filter.pop();
                                self.update_filtered();
                            }
                            _ => {}
                        }
                    }
                }
                if last_tick.elapsed() >= config.tick_rate() {
                    last_tick = Instant::now();
                }
            }
        }
    }
}
