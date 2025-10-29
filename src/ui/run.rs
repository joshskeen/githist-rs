pub mod app {
    use crate::git::branching::{change_branch, delete_branch, get_branch_names, Config};
    use crate::ui::gui::restore_terminal;
    use crate::App;
    use crossterm::event;
    use crossterm::event::{Event, KeyCode, KeyModifiers};
    use ratatui::backend::CrosstermBackend;
    use ratatui::Terminal;
    use std::io;
    use std::io::Stdout;
    use std::time::{Duration, Instant};

    impl App {
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

                let timeout = config
                    .tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout)? {
                    if let Event::Key(key) = event::read()? {
                        if let Some(branch_name) = self.delete_confirmation.clone() {
                            match key.code {
                                KeyCode::Char('Y') | KeyCode::Char('y') => {
                                    self.delete_confirmation = None;
                                    match delete_branch(config, &branch_name) {
                                        Ok(_) => match get_branch_names(config) {
                                            Ok(branches) => {
                                                self.set_branches(branches);
                                                self.select_first_item_if_none();
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
                        match key.code {
                            KeyCode::Enter => {
                                let branch_name = self.get_selected_branch_name();
                                match branch_name {
                                    Ok(name) => {
                                        // deal with this if there's an error.
                                        let status = format!("{}{}", "switching to branch: ", name);
                                        self.update_with_status(terminal, status);
                                        match change_branch(config, &name) {
                                            Ok(_) => {}
                                            Err(error) => {
                                                // restore the terminal, then print the error if one occurred
                                                // while changing branch.
                                                restore_terminal(terminal)
                                                    .expect("couldn't restore!");
                                                eprintln!(
                                                    "couldn't change branch. reason: {error}"
                                                );
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
                            KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                                let branch_name = self.get_selected_branch_name();
                                match branch_name {
                                    Ok(name) => {
                                        self.delete_confirmation = Some(name.clone());
                                        let status = format!(
                                            "confirm deleting branch {}? press Y to delete or N to cancel",
                                            name
                                        );
                                        self.update_with_status_preserve_filter(terminal, status);
                                    }
                                    Err(_) => {
                                        let status = "no selection, nothing to delete!".to_string();
                                        self.update_with_status_preserve_filter(terminal, status);
                                    }
                                }
                            }
                            KeyCode::Left => self.items.unselect(),
                            KeyCode::Down => self.items.next(),
                            KeyCode::Up => self.items.previous(),
                            KeyCode::Backspace => {
                                self.filter.pop();
                                self.update_filtered();
                            }
                            // update the filter used to limit the vec of branches shown
                            KeyCode::Char(c)
                                if key.modifiers.is_empty()
                                    || key.modifiers == KeyModifiers::SHIFT =>
                            {
                                self.filter.push(c);
                                self.update_filtered();
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
    }
}
