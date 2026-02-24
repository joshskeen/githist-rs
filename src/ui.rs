pub mod run;

pub mod gui {
    use crate::App;
    use crossterm::execute;
    use crossterm::terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    };
    use pad::PadStr;
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

    impl App {
        pub(crate) fn ui(&mut self, f: &mut Frame) {
            let chunks = Layout::default()
                .constraints(
                    [
                        Constraint::Percentage(96),
                        Constraint::Percentage(2),
                        Constraint::Percentage(2),
                    ]
                    .as_ref(),
                )
                .direction(Direction::Vertical)
                .split(f.size());

            // get the longest of all the branch names including ones not currently displayed necessarily.
            let largest_string_len = self
                .items
                .items
                .iter()
                .map(|x| x.branch_name.len())
                .max()
                .unwrap_or(0);

            let items: Vec<ListItem> = self
                .items
                .filtered
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(|branch_info| {
                    let head_marker = if branch_info.is_head { "* " } else { "  " };
                    let branch_and_padding =
                        branch_info.branch_name.pad_to_width(largest_string_len);
                    let remote_info = branch_info
                        .remote_tracking
                        .as_deref()
                        .map_or(String::new(), |r| format!(" [{r}]"));

                    let mut spans = vec![
                        Span::styled(
                            head_marker,
                            if branch_info.is_head {
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            },
                        ),
                        Span::raw(format!(
                            "{}   changed: {}",
                            branch_and_padding, branch_info.time_ago
                        )),
                    ];
                    if !remote_info.is_empty() {
                        spans.push(Span::styled(
                            remote_info,
                            Style::default().fg(Color::Cyan),
                        ));
                    }

                    ListItem::new(Line::from(spans))
                        .style(Style::default().fg(Color::Black).bg(Color::White))
                })
                .collect();

            let count_info = if self.filtered_len() == self.total_len() {
                format!("{} branches", self.total_len())
            } else {
                format!("{}/{} branches", self.filtered_len(), self.total_len())
            };

            let title = format!("choose recent branch  ({count_info})");
            let items = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::LightGreen)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol(">> ");

            let instructions_text =
                "q/Esc: quit | j/k/↓/↑: navigate | ↩: switch branch | Shift+D: delete | /: filter | g/G: first/last | PgUp/PgDn: page";
            let instructions_para = Paragraph::new(instructions_text)
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: true });

            // list of branches
            f.render_stateful_widget(items, chunks[0], &mut self.items.state);

            // instructions
            f.render_widget(instructions_para, chunks[1]);

            // status bar: show filter, pending status, or filter mode indicator
            let status_text = if !self.pending.is_empty() {
                format!("status: {}", self.pending)
            } else if self.filter_mode {
                format!("filter: {}_", self.filter)
            } else if !self.filter.is_empty() {
                format!("filter: {} (press / to edit, Backspace to clear)", self.filter)
            } else {
                String::new()
            };

            if !status_text.is_empty() {
                let status_para = Paragraph::new(status_text)
                    .block(Block::default().borders(Borders::NONE))
                    .wrap(Wrap { trim: true });
                f.render_widget(status_para, chunks[2]);
            }
        }
    }
}
