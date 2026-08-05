pub mod app {
    use crate::acp_sessions::{current_session_from_env, list_candidates, save_link};
    use crate::agent_store::{store_path, LinkedSession};
    use crate::git::branching::{local_branch_name, BranchInfo, Config, Repo};
    use crate::{
        resume_cwd, should_enter_resume_picker, skip_resume_exit, App, AppExit, Mode, PostNav,
        SwitchIntent, TuiTerminal,
    };
    use crossterm::event;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::io;
    use std::time::{Duration, Instant};

    const PAGE_SIZE: usize = 10;

    enum Outcome {
        Stay,
        Exit(AppExit),
    }

    impl App {
        /// Runs the event loop. Returns how the session ended after the terminal
        /// is restored.
        ///
        /// # Errors
        ///
        /// Will return `Err` if drawing or event polling failed.
        pub fn run_app(
            &mut self,
            config: &Config,
            repo: &mut Repo,
            terminal: &mut TuiTerminal,
        ) -> io::Result<AppExit> {
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
                            Mode::LinkAgent {
                                branch_name,
                                candidates,
                                selected,
                                paste_buffer,
                            } => self.handle_link_agent(
                                key,
                                branch_name,
                                candidates,
                                selected,
                                paste_buffer,
                            ),
                            Mode::ResumeAgent {
                                branch_name,
                                sessions,
                                selected,
                                post_nav,
                            } => self.handle_resume_agent(
                                key,
                                branch_name,
                                sessions,
                                selected,
                                post_nav,
                            ),
                            Mode::Filter => self.handle_filter_mode(key, repo, terminal),
                            Mode::Normal => self.handle_normal_mode(key, repo, terminal),
                        };
                        if let Outcome::Exit(exit) = outcome {
                            return Ok(exit);
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
            terminal: &mut TuiTerminal,
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
                        self.after_switch_resume = false;
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
                    self.after_switch_resume = false;
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
                        Ok(()) => Outcome::Exit(AppExit::Farewell(format!(
                            "created and switched to branch '{name}'"
                        ))),
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

        fn handle_link_agent(
            &mut self,
            key: KeyEvent,
            branch_name: String,
            candidates: Vec<LinkedSession>,
            selected: usize,
            paste_buffer: String,
        ) -> Outcome {
            match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    self.pending.clear();
                    Outcome::Stay
                }
                KeyCode::Up | KeyCode::Char('k') if !candidates.is_empty() => {
                    let selected = if selected == 0 {
                        candidates.len() - 1
                    } else {
                        selected - 1
                    };
                    self.mode = Mode::LinkAgent {
                        branch_name,
                        candidates,
                        selected,
                        paste_buffer,
                    };
                    Outcome::Stay
                }
                KeyCode::Down | KeyCode::Char('j') if !candidates.is_empty() => {
                    let selected = (selected + 1) % candidates.len();
                    self.mode = Mode::LinkAgent {
                        branch_name,
                        candidates,
                        selected,
                        paste_buffer,
                    };
                    Outcome::Stay
                }
                KeyCode::Enter => {
                    let session = if candidates.is_empty() {
                        let session_id = paste_buffer.trim().to_string();
                        if session_id.is_empty() {
                            self.pending = "enter a session id to link".to_string();
                            self.mode = Mode::LinkAgent {
                                branch_name,
                                candidates,
                                selected,
                                paste_buffer,
                            };
                            return Outcome::Stay;
                        }
                        LinkedSession {
                            session_id,
                            title: None,
                            linked_at: chrono::Utc::now().to_rfc3339(),
                        }
                    } else {
                        match candidates.get(selected).cloned() {
                            Some(session) => session,
                            None => {
                                self.pending = "no session selected".to_string();
                                self.mode = Mode::LinkAgent {
                                    branch_name,
                                    candidates,
                                    selected,
                                    paste_buffer,
                                };
                                return Outcome::Stay;
                            }
                        }
                    };
                    match save_link(
                        &mut self.agent_store,
                        &self.repo_id,
                        &branch_name,
                        &session,
                    ) {
                        Ok(()) => {
                            self.mode = Mode::Normal;
                            let label = session
                                .title
                                .as_deref()
                                .unwrap_or(&session.session_id);
                            self.pending =
                                format!("linked agent session to '{branch_name}': {label}");
                        }
                        Err(error) => {
                            self.pending = format!("couldn't save agent link: {error}");
                            self.mode = Mode::LinkAgent {
                                branch_name,
                                candidates,
                                selected,
                                paste_buffer,
                            };
                        }
                    }
                    Outcome::Stay
                }
                KeyCode::Backspace if candidates.is_empty() => {
                    let mut paste_buffer = paste_buffer;
                    paste_buffer.pop();
                    self.mode = Mode::LinkAgent {
                        branch_name,
                        candidates,
                        selected,
                        paste_buffer,
                    };
                    Outcome::Stay
                }
                KeyCode::Char(c)
                    if candidates.is_empty()
                        && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) =>
                {
                    let mut paste_buffer = paste_buffer;
                    paste_buffer.push(c);
                    self.mode = Mode::LinkAgent {
                        branch_name,
                        candidates,
                        selected,
                        paste_buffer,
                    };
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_resume_agent(
            &mut self,
            key: KeyEvent,
            branch_name: String,
            mut sessions: Vec<LinkedSession>,
            selected: usize,
            post_nav: PostNav,
        ) -> Outcome {
            let mut selected = selected;
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.mode = Mode::Normal;
                    Outcome::Exit(skip_resume_exit(post_nav))
                }
                KeyCode::Up | KeyCode::Char('k') if !sessions.is_empty() => {
                    if selected == 0 {
                        selected = sessions.len() - 1;
                    } else {
                        selected -= 1;
                    }
                    self.mode = Mode::ResumeAgent {
                        branch_name,
                        sessions,
                        selected,
                        post_nav,
                    };
                    Outcome::Stay
                }
                KeyCode::Down | KeyCode::Char('j') if !sessions.is_empty() => {
                    selected = (selected + 1) % sessions.len();
                    self.mode = Mode::ResumeAgent {
                        branch_name,
                        sessions,
                        selected,
                        post_nav,
                    };
                    Outcome::Stay
                }
                KeyCode::Enter if !sessions.is_empty() => {
                    let session = &sessions[selected];
                    let cwd = resume_cwd(&post_nav, &self.repo_cwd);
                    Outcome::Exit(AppExit::ResumeAgent {
                        session_id: session.session_id.clone(),
                        cwd,
                    })
                }
                KeyCode::Char('u') | KeyCode::Char('U') if !sessions.is_empty() => {
                    let session_id = sessions[selected].session_id.clone();
                    self.agent_store.unlink(&branch_name, &session_id);
                    if let Err(error) = self.agent_store.save(&store_path(&self.repo_id)) {
                        self.pending = format!("couldn't save agent link store: {error}");
                    } else {
                        self.pending.clear();
                    }
                    sessions.retain(|s| s.session_id != session_id);
                    if sessions.is_empty() {
                        self.mode = Mode::Normal;
                        Outcome::Exit(skip_resume_exit(post_nav))
                    } else {
                        let selected = selected.min(sessions.len() - 1);
                        self.mode = Mode::ResumeAgent {
                            branch_name,
                            sessions,
                            selected,
                            post_nav,
                        };
                        Outcome::Stay
                    }
                }
                _ => Outcome::Stay,
            }
        }

        fn handle_filter_mode(
            &mut self,
            key: KeyEvent,
            repo: &mut Repo,
            terminal: &mut TuiTerminal,
        ) -> Outcome {
            match key.code {
                KeyCode::Esc => {
                    self.mode = Mode::Normal;
                    Outcome::Stay
                }
                KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    self.try_switch_selected(repo, terminal, SwitchIntent::Enter)
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
            terminal: &mut TuiTerminal,
        ) -> Outcome {
            match key.code {
                KeyCode::Enter => self.try_switch_selected(repo, terminal, SwitchIntent::Enter),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    Outcome::Exit(AppExit::Quit)
                }
                KeyCode::Char('a') if key.modifiers.is_empty() => {
                    match self.get_selected_branch_info() {
                        Ok(info) => {
                            let has_links = self
                                .agent_store
                                .sessions_for(&info.branch_name)
                                .is_some_and(|sessions| !sessions.is_empty());
                            if !has_links {
                                self.pending =
                                    "no agent linked; Shift+A to link".to_string();
                                Outcome::Stay
                            } else {
                                self.after_switch_resume = true;
                                self.request_switch(
                                    info,
                                    repo,
                                    terminal,
                                    SwitchIntent::ThenResume,
                                )
                            }
                        }
                        Err(_) => {
                            self.pending = "no selection, nothing to do!".to_string();
                            Outcome::Stay
                        }
                    }
                }
                KeyCode::Char('A') if key.modifiers == KeyModifiers::SHIFT => {
                    match self.get_selected_branch_info() {
                        Ok(info) => {
                            let mut candidates = list_candidates(&self.repo_cwd, 20);
                            if let Some(current) = current_session_from_env() {
                                candidates.retain(|c| c.session_id != current.session_id);
                                candidates.insert(0, current);
                            }
                            self.mode = Mode::LinkAgent {
                                branch_name: info.branch_name,
                                candidates,
                                selected: 0,
                                paste_buffer: String::new(),
                            };
                            self.pending.clear();
                            Outcome::Stay
                        }
                        Err(_) => {
                            self.pending = "no selection, nothing to link!".to_string();
                            Outcome::Stay
                        }
                    }
                }
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
                            } else if let Some(path) = info.worktree_path {
                                self.pending = format!(
                                    "can't delete '{}': checked out in {path}",
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
                            Some(info) => self.request_switch(
                                info,
                                repo,
                                terminal,
                                SwitchIntent::Enter,
                            ),
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
        fn try_switch_selected(
            &mut self,
            repo: &mut Repo,
            terminal: &mut TuiTerminal,
            intent: SwitchIntent,
        ) -> Outcome {
            match self.get_selected_branch_info() {
                Ok(info) => self.request_switch(info, repo, terminal, intent),
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

        fn enter_resume_agent(&mut self, branch_name: String, post_nav: PostNav) -> Outcome {
            self.after_switch_resume = false;
            let sessions = self
                .agent_store
                .sessions_for(&branch_name)
                .map(<[LinkedSession]>::to_vec)
                .unwrap_or_default();
            self.mode = Mode::ResumeAgent {
                branch_name,
                sessions,
                selected: 0,
                post_nav,
            };
            self.pending.clear();
            Outcome::Stay
        }

        /// Worktree-held branches exit with the path; dirty trees prompt for
        /// stash/bring/cancel; clean trees switch directly.
        fn request_switch(
            &mut self,
            target: BranchInfo,
            repo: &mut Repo,
            terminal: &mut TuiTerminal,
            intent: SwitchIntent,
        ) -> Outcome {
            if target.is_head {
                self.after_switch_resume = false;
                self.pending = format!("already on branch '{}'", target.branch_name);
                return Outcome::Stay;
            }
            if let Some(path) = target.worktree_path {
                if should_enter_resume_picker(intent, self.after_switch_resume) {
                    return self.enter_resume_agent(
                        target.branch_name,
                        PostNav::WorktreePath(path),
                    );
                }
                self.after_switch_resume = false;
                return Outcome::Exit(AppExit::WorktreePath(path));
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
                    self.after_switch_resume = false;
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
                    if self.after_switch_resume {
                        return self.enter_resume_agent(
                            target.branch_name.clone(),
                            PostNav::Farewell(message),
                        );
                    }
                    Outcome::Exit(AppExit::Farewell(message))
                }
                Err(error) => {
                    self.after_switch_resume = false;
                    self.pending = format!("couldn't switch: {error}");
                    Outcome::Stay
                }
            }
        }
    }
}
