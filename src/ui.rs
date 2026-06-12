pub mod run;

pub mod gui {
    use crate::{App, Mode};
    use crossterm::execute;
    use crossterm::terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    };
    use ratatui::backend::CrosstermBackend;
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
    use ratatui::{Frame, Terminal};
    use std::io;
    use std::io::Stdout;

    pub fn setup_terminal() -> Terminal<CrosstermBackend<Stdout>> {
        enable_raw_mode().expect("failed to enter raw mode!");
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).expect("failed to setup terminal!");
        let backend = CrosstermBackend::new(stdout);
        Terminal::new(backend).expect("failed to instance terminal")
    }

    pub fn restore_terminal(
        terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ) -> Result<(), io::Error> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
        Ok(())
    }

    /// Render `name` as spans, styling the fuzzy-matched `positions` with
    /// `highlight`, padded with trailing spaces to `pad_to` chars.
    fn highlighted_spans(
        name: &str,
        positions: &[usize],
        base: Style,
        highlight: Style,
        pad_to: usize,
    ) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut run = String::new();
        let mut run_highlighted = false;
        for (i, ch) in name.chars().enumerate() {
            let is_highlighted = positions.binary_search(&i).is_ok();
            if is_highlighted != run_highlighted && !run.is_empty() {
                let style = if run_highlighted { highlight } else { base };
                spans.push(Span::styled(std::mem::take(&mut run), style));
            }
            run_highlighted = is_highlighted;
            run.push(ch);
        }
        if !run.is_empty() {
            let style = if run_highlighted { highlight } else { base };
            spans.push(Span::styled(run, style));
        }
        let len = name.chars().count();
        if pad_to > len {
            spans.push(Span::raw(" ".repeat(pad_to - len)));
        }
        spans
    }

    impl App {
        pub(crate) fn ui(&mut self, f: &mut Frame) {
            let chunks = Layout::default()
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .direction(Direction::Vertical)
                .split(f.area());

            // get the longest of all the branch names including ones not currently displayed necessarily.
            let largest_string_len = self
                .items
                .items
                .iter()
                .map(|x| x.branch_name.chars().count())
                .max()
                .unwrap_or(0);

            let items: Vec<ListItem> = self
                .items
                .filtered
                .iter()
                .map(|entry| {
                    let branch_info = &self.items.items[entry.index];
                    let base_style = if branch_info.is_remote {
                        Style::default().add_modifier(Modifier::DIM)
                    } else {
                        Style::default()
                    };
                    let match_style = base_style.fg(Color::Magenta).add_modifier(Modifier::BOLD);

                    let mut spans = vec![Span::styled(
                        if branch_info.is_head { "* " } else { "  " },
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )];
                    spans.extend(highlighted_spans(
                        &branch_info.branch_name,
                        &entry.positions,
                        base_style,
                        match_style,
                        largest_string_len,
                    ));
                    spans.push(Span::styled(
                        format!("   {}", branch_info.time_ago),
                        base_style,
                    ));
                    let summary: String = branch_info.summary.chars().take(40).collect();
                    if !summary.is_empty() {
                        spans.push(Span::styled(
                            format!("   {summary}"),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                    if branch_info.has_stash {
                        spans.push(Span::styled(
                            "  \u{2691} stashed",
                            Style::default().fg(Color::Yellow),
                        ));
                    }
                    if let Some(remote) = branch_info.remote_tracking.as_deref() {
                        spans.push(Span::styled(
                            format!(" [{remote}]"),
                            Style::default().fg(Color::Cyan),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();

            let count_info = if self.filtered_len() == self.total_len() {
                format!("{} branches", self.total_len())
            } else {
                format!("{}/{} branches", self.filtered_len(), self.total_len())
            };

            let title = format!("choose recent branch  ({count_info})");
            let items = List::new(items)
                .block(Block::default().borders(Borders::ALL).title(title))
                .highlight_style(
                    Style::default()
                        .bg(Color::LightGreen)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            let instructions_text = "q/Esc: quit | j/k/\u{2193}/\u{2191}: navigate | \u{21a9}: switch | -: previous branch | Shift+D: delete | /: filter | g/G: first/last";
            let instructions_para = Paragraph::new(instructions_text)
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: true });

            // list of branches
            f.render_stateful_widget(items, chunks[0], &mut self.items.state);

            // instructions
            f.render_widget(instructions_para, chunks[1]);

            // status bar: pending status, or a mode-specific prompt
            let status_text = self.status_line();
            if !status_text.is_empty() {
                let status_para = Paragraph::new(status_text)
                    .block(Block::default().borders(Borders::NONE))
                    .wrap(Wrap { trim: true });
                f.render_widget(status_para, chunks[2]);
            }
        }

        fn status_line(&self) -> String {
            if !self.pending.is_empty() {
                return format!("status: {}", self.pending);
            }
            match &self.mode {
                Mode::Filter => {
                    if self.filtered_len() == 0 && !self.filter.is_empty() {
                        format!(
                            "filter: {}_  (no matches \u{2014} press Enter to create this branch)",
                            self.filter
                        )
                    } else {
                        format!("filter: {}_", self.filter)
                    }
                }
                Mode::ConfirmDelete { branch_name, merged } => {
                    if *merged {
                        format!("delete branch '{branch_name}'? [y/n]")
                    } else {
                        format!(
                            "delete branch '{branch_name}'? NOT merged into HEAD \u{2014} commits may be lost [y/n]"
                        )
                    }
                }
                Mode::DirtyPrompt { target } => format!(
                    "working tree is dirty \u{2014} switch to '{}': [s]tash changes / [b]ring along / [c]ancel",
                    target.branch_name
                ),
                Mode::ConfirmCreate { name } => {
                    format!("create branch '{name}' and switch to it? [y/n]")
                }
                Mode::Normal => {
                    if self.filter.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "filter: {} (press / to edit, Backspace to clear)",
                            self.filter
                        )
                    }
                }
            }
        }
    }
}
