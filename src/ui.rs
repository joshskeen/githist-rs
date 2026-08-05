pub mod run;

use ratatui::style::{Color, Modifier, Style};

/// Gutter marker for a branch row.
///
/// Priority: head (`*`) > worktree (`W`) > agent (`a`) > blank.
pub fn branch_gutter(is_head: bool, has_worktree: bool, has_agent: bool) -> (&'static str, Style) {
    if is_head {
        (
            "* ",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else if has_worktree {
        ("W ", Style::default().fg(Color::Magenta))
    } else if has_agent {
        (
            "a ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::DIM),
        )
    } else {
        ("  ", Style::default())
    }
}

pub mod gui {
    use crate::ui::branch_gutter;
    use crate::path_display::format_worktree_path;
    use crate::{App, Mode, TuiTerminal};
    use crossterm::execute;
    use crossterm::terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    };
    use ratatui::layout::{Constraint, Direction, Layout};
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
    use ratatui::Frame;
    use std::fs::OpenOptions;
    use std::io::{self, Write};

    /// Open the controlling terminal for the TUI so stdout stays free for path emission.
    ///
    /// # Errors
    ///
    /// Returns an error if `/dev/tty` cannot be opened or the terminal cannot be set up.
    pub fn setup_terminal() -> io::Result<TuiTerminal> {
        let mut tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        enable_raw_mode()?;
        execute!(tty, EnterAlternateScreen)?;
        let backend = ratatui::backend::CrosstermBackend::new(tty);
        ratatui::Terminal::new(backend).map_err(io::Error::other)
    }

    pub fn restore_terminal(terminal: &mut TuiTerminal) -> Result<(), io::Error> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        // Ensure leave-alternate-screen is flushed before we print to stdout.
        terminal.backend_mut().flush()?;
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

            let largest_string_len = self
                .items
                .items
                .iter()
                .map(|x| x.branch_name.chars().count())
                .max()
                .unwrap_or(0);

            let row_width = chunks[0].width as usize;
            let path_budget = row_width.saturating_sub(largest_string_len + 60);

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

                    let has_agent = self
                        .agent_store
                        .sessions_for(&branch_info.branch_name)
                        .is_some_and(|sessions| !sessions.is_empty());
                    let (marker, marker_style) = branch_gutter(
                        branch_info.is_head,
                        branch_info.worktree_path.is_some(),
                        has_agent,
                    );

                    let mut spans = vec![Span::styled(marker, marker_style)];
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
                    if let Some(path) = branch_info.worktree_path.as_deref() {
                        if path_budget >= 8 {
                            let shown = format_worktree_path(path, path_budget);
                            spans.push(Span::styled(
                                format!("  {shown}"),
                                Style::default().fg(Color::DarkGray),
                            ));
                        }
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

            let base_help = "q/Esc: quit | j/k/\u{2193}/\u{2191}: navigate | \u{21a9}: switch or open worktree | -: previous branch | Shift+D: delete | /: filter | g/G: first/last";
            let show_agent_help = self.agent_store.has_any_links()
                || self
                    .get_selected_branch_info()
                    .ok()
                    .is_some_and(|info| {
                        self.agent_store
                            .sessions_for(&info.branch_name)
                            .is_some_and(|sessions| !sessions.is_empty())
                    });
            let instructions_text = if show_agent_help {
                format!("{base_help} | a: resume agent | A: link session")
            } else {
                base_help.to_string()
            };
            let instructions_para = Paragraph::new(instructions_text)
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: true });

            f.render_stateful_widget(items, chunks[0], &mut self.items.state);
            f.render_widget(instructions_para, chunks[1]);

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
                    if !self.filter.is_empty() {
                        format!(
                            "filter: {} (press / to edit, Backspace to clear)",
                            self.filter
                        )
                    } else if let Ok(info) = self.get_selected_branch_info() {
                        let session_count = self
                            .agent_store
                            .sessions_for(&info.branch_name)
                            .map_or(0, |sessions| sessions.len());
                        let mut parts = Vec::new();
                        if let Some(path) = &info.worktree_path {
                            parts.push(format!("worktree: {path}"));
                        }
                        if session_count > 0 {
                            let label = if session_count == 1 {
                                "agent: 1 linked session".to_string()
                            } else {
                                format!("agent: {session_count} linked sessions")
                            };
                            parts.push(label);
                        }
                        parts.join(" | ")
                    } else {
                        String::new()
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::branch_gutter;
    use ratatui::style::{Color, Modifier, Style};

    #[test]
    fn branch_gutter_head_beats_worktree_and_agent() {
        let (marker, style) = branch_gutter(true, true, true);
        assert_eq!(marker, "* ");
        assert_eq!(style.fg, Some(Color::Yellow));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn branch_gutter_worktree_beats_agent() {
        let (marker, style) = branch_gutter(false, true, true);
        assert_eq!(marker, "W ");
        assert_eq!(style.fg, Some(Color::Magenta));
    }

    #[test]
    fn branch_gutter_agent_when_no_head_or_worktree() {
        let (marker, style) = branch_gutter(false, false, true);
        assert_eq!(marker, "a ");
        assert_eq!(style.fg, Some(Color::Cyan));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn branch_gutter_blank_when_nothing() {
        let (marker, style) = branch_gutter(false, false, false);
        assert_eq!(marker, "  ");
        assert_eq!(style, Style::default());
    }
}
