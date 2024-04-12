pub mod app {
    use crate::git::branching::{change_branch, Config};
    use crate::ui::gui::restore_terminal;
    use crate::App;
    use crossterm::event;
    use crossterm::event::{Event, KeyCode};
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
                            KeyCode::Left => self.items.unselect(),
                            KeyCode::Down => self.items.next(),
                            KeyCode::Up => self.items.previous(),
                            KeyCode::Backspace => {
                                self.filter.pop();
                                self.update_filtered();
                            }
                            // update the filter used to limit the vec of branches shown
                            KeyCode::Char(c) => {
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
