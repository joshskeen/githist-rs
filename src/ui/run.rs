pub mod app {
    use crate::git::branching::{local_branch_name, BranchInfo, Config, Repo};
    use crate::{App, Mode};
    use crossterm::event;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::CrosstermBackend;
    use ratatui::Terminal;
    use std::io;
    use std::io::Stdout;
    use std::time::{Duration, Instant};

    const PAGE_SIZE: usize = 10;

    type Term = Terminal<CrosstermBackend<Stdout>>;

    enum Outcome {
        Stay,
        Exit(Option<String>),
    }

    impl App {
        /// Runs the event loop. Returns a farewell message to print after the
        /// terminal is restored (`None` when the user just quit).
        ///
        /// # Errors
        ///
        /// Will return `Err` if drawing or event polling failed.
        pub fn run_app(
            &mut self,
            config: &Config,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> io::Result<Option<String>> {
            let mut last_tick = Instant::now();
            loop {
                terminal.draw(|f| self.ui(f))?;

                let timeout = config
                    .tick_rate()
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));
                if event::poll(timeout)? {
                    if let Event::Key(key) = event::read()? {
                        let outcome = match self.mode.clone() {
                            Mode::ConfirmDelete { branch_name, .. } => {
                                self.handle_confirm_delete(key, &branch_name, repo);
                                Outcome::Stay
                            }
                            Mode::DirtyPrompt { target } => {
                                self.handle_dirty_prompt(key, &target, repo, terminal)
                            }
                            Mode::ConfirmCreate { name } => {
                                self.handle_confirm_create(key, &name, repo)
                            }
                            Mode::Filter => self.handle_filter_mode(key, repo, terminal),
                            Mode::Normal => self.handle_normal_mode(key, repo, terminal),
                        };
                        if let Outcome::Exit(message) = outcome {
                            return Ok(message);
                        }
                    }
                }
                if last_tick.elapsed() >= config.tick_rate() {
                    last_tick = Instant::now();
                }
            }
        }

        fn handle_confirm_delete(&mut self, key: KeyEvent, branch_name: &str, repo: &mut Repo) {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.mode = Mode::Normal;
                    let selected_index = self.items.state.selected();
                    match repo.delete_branch(branch_name) {
                        Ok(()) => match repo.get_branch_names() {
                            Ok(branches) => {
                                self.set_branches(branches);
                                if let Some(idx) = selected_index {
                                    let new_len = self.filtered_len();
                                    if new_len > 0 {
                                        self.items.state.select(Some(idx.min(new_len - 1)));
                                    }
                                }
                                self.pending = format!("deleted branch: {branch_name}");
                            }
                            Err(error) => {
                                self.pending =
                                    format!("deleted branch but failed to refresh list: {error}");
                            }
                        },
                        Err(error) => {
                            self.pending = format!("couldn't delete branch {branch_name}: {error}");
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc | KeyCode::Backspace => {
                    self.mode = Mode::Normal;
                    self.pending.clear();
                }
                _ => {}
            }
        }

        fn handle_dirty_prompt(
            &mut self,
            key: KeyEvent,
            target: &BranchInfo,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            match key.code {
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.mode = Mode::Normal;
                    self.show_status(
                        terminal,
                        format!("stashing changes, then switching to {}", target.branch_name),
                    );
                    if let Err(error) = repo.stash_changes(&target.branch_name) {
                        self.pending = format!("couldn't stash changes: {error}");
                        return Outcome::Stay;
                    }
                    self.perform_switch(target, repo)
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    self.mode = Mode::Normal;
                    self.perform_switch(target, repo)
                }
                KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_confirm_create(&mut self, key: KeyEvent, name: &str, repo: &mut Repo) -> Outcome {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    match repo.create_branch(name) {
                        Ok(()) => {
                            Outcome::Exit(Some(format!("created and switched to branch '{name}'")))
                        }
                        Err(error) => {
                            self.pending = format!("couldn't create branch '{name}': {error}");
                            Outcome::Stay
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_filter_mode(
            &mut self,
            key: KeyEvent,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Outcome::Stay
                }
                KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    self.try_switch_selected(repo, terminal)
                }
                KeyCode::Up => {
                    self.items.previous();
                    Outcome::Stay
                }
                KeyCode::Down => {
                    self.items.next();
                    Outcome::Stay
                }
                KeyCode::Backspace => {
                    if self.filter.pop().is_none() {
                        self.mode = Mode::Normal;
                    }
                    self.update_filtered();
                    Outcome::Stay
                }
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.filter.push(c);
                    self.update_filtered();
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_normal_mode(
            &mut self,
            key: KeyEvent,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            match key.code {
                KeyCode::Enter => self.try_switch_selected(repo, terminal),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Outcome::Exit(None),
                KeyCode::Char('D') if key.modifiers == KeyModifiers::SHIFT => {
                    match self.get_selected_branch_info() {
                        Ok(info) => {
                            if info.is_head {
                                self.pending = format!(
                                    "can't delete '{}': it is the current branch",
                                    info.branch_name
                                );
                            } else if info.is_remote {
                                self.pending = format!(
                                    "'{}' is a remote branch; githist only deletes local branches",
                                    info.branch_name
                                );
                            } else {
                                let merged =
                                    repo.is_merged_into_head(&info.branch_name).unwrap_or(false);
                                self.pending.clear();
                                self.mode = Mode::ConfirmDelete {
                                    branch_name: info.branch_name,
                                    merged,
                                };
                            }
                        }
                        Err(_) => {
                            self.pending = "no selection, nothing to delete!".to_string();
                        }
                    }
                    Outcome::Stay
                }
                KeyCode::Char('/') => {
                    self.mode = Mode::Filter;
                    self.pending.clear();
                    Outcome::Stay
                }
                KeyCode::Char('-') => match repo.previous_branch() {
                    Some(prev) => {
                        let info = self
                            .items
                            .items
                            .iter()
                            .find(|b| !b.is_remote && b.branch_name == prev)
                            .cloned();
                        match info {
                            Some(info) => self.request_switch(info, repo, terminal),
                            None => {
                                self.pending =
                                    format!("previous branch '{prev}' is not in the list");
                                Outcome::Stay
                            }
                        }
                    }
                    None => {
                        self.pending = "no previous branch in the reflog".to_string();
                        Outcome::Stay
                    }
                },
                KeyCode::Down | KeyCode::Char('j') => {
                    self.items.next();
                    Outcome::Stay
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.items.previous();
                    Outcome::Stay
                }
                KeyCode::PageDown => {
                    self.items.page_down(PAGE_SIZE);
                    Outcome::Stay
                }
                KeyCode::PageUp => {
                    self.items.page_up(PAGE_SIZE);
                    Outcome::Stay
                }
                KeyCode::Home | KeyCode::Char('g') => {
                    self.items.go_to_first();
                    Outcome::Stay
                }
                KeyCode::End | KeyCode::Char('G') => {
                    self.items.go_to_last();
                    Outcome::Stay
                }
                KeyCode::Left => {
                    self.items.unselect();
                    Outcome::Stay
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.update_filtered();
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        /// Switch to the selected branch; with no selection but a non-matching
        /// filter, offer to create a branch named after the filter text.
        fn try_switch_selected(&mut self, repo: &mut Repo, terminal: &mut Term) -> Outcome {
            match self.get_selected_branch_info() {
                Ok(info) => self.request_switch(info, repo, terminal),
                Err(_) => {
                    if !self.filter.is_empty() && self.filtered_len() == 0 {
                        self.mode = Mode::ConfirmCreate {
                            name: self.filter.clone(),
                        };
                    } else {
                        self.pending = "no selection, nothing to do!".to_string();
                    }
                    Outcome::Stay
                }
            }
        }

        /// Dirty trees prompt for stash/bring/cancel; clean trees switch directly.
        fn request_switch(
            &mut self,
            target: BranchInfo,
            repo: &mut Repo,
            terminal: &mut Term,
        ) -> Outcome {
            if target.is_head {
                self.pending = format!("already on branch '{}'", target.branch_name);
                return Outcome::Stay;
            }
            match repo.is_dirty() {
                Ok(true) => {
                    self.mode = Mode::DirtyPrompt { target };
                    Outcome::Stay
                }
                Ok(false) => {
                    self.show_status(
                        terminal,
                        format!("switching to branch: {}", target.branch_name),
                    );
                    self.perform_switch(&target, repo)
                }
                Err(error) => {
                    self.pending = format!("couldn't check working tree status: {error}");
                    Outcome::Stay
                }
            }
        }

        /// Checkout (creating a tracking branch for remotes), then pop any
        /// githist stash that was taken when this branch was last left.
        fn perform_switch(&mut self, target: &BranchInfo, repo: &mut Repo) -> Outcome {
            let result = if target.is_remote {
                repo.checkout_remote(&target.branch_name).map(|local| {
                    format!(
                        "switched to new branch '{local}' tracking {}",
                        target.branch_name
                    )
                })
            } else {
                repo.change_branch(&target.branch_name)
                    .map(|()| format!("switched to branch '{}'", target.branch_name))
            };
            match result {
                Ok(mut message) => {
                    let stash_branch = if target.is_remote {
                        local_branch_name(&target.branch_name).to_string()
                    } else {
                        target.branch_name.clone()
                    };
                    if let Some(index) = repo.find_githist_stash(&stash_branch) {
                        match repo.pop_stash(index) {
                            Ok(()) => message.push_str("; restored stashed changes"),
                            Err(error) => message.push_str(&format!(
                                "; couldn't restore stashed changes ({error}) \u{2014} they remain in the stash list"
                            )),
                        }
                    }
                    Outcome::Exit(Some(message))
                }
                Err(error) => {
                    self.pending = format!("couldn't switch: {error}");
                    Outcome::Stay
                }
            }
        }
    }
}
