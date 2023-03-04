pub mod run;

pub mod gui {
    use std::io;
    use std::io::Stdout;
    use crossterm::execute;
    use tui::backend::{Backend, CrosstermBackend};
    use tui::{Frame, Terminal};
    use crossterm::{
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use pad::PadStr;
    use tui::layout::{Constraint, Direction, Layout};
    use tui::style::{Color, Modifier, Style};
    use tui::text::Spans;
    use tui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
    use crate::App;

    pub fn setup_terminal() -> Terminal<CrosstermBackend<Stdout>> {
        enable_raw_mode().expect("failed to enter raw mode!");
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen).expect("failed to setup terminal!");
        let backend = CrosstermBackend::new(stdout);
        Terminal::new(backend).expect("failed to instance terminal")
    }

    pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), io::Error> {
        disable_raw_mode()?;
        execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
            )?;
        Ok(())
    }


    impl App {
        pub(crate) fn ui<B: Backend>(&mut self, f: &mut Frame<B>) {
            let chunks = Layout::default()
                .constraints([Constraint::Percentage(96), Constraint::Percentage(2), Constraint::Percentage(2)].as_ref())
                .direction(Direction::Vertical)
                .split(f.size());

            let largest_string_len = self.items.filtered.clone().unwrap()
                .iter()
                .map(|x| x.branch_name.len())
                .max().unwrap();

            let items: Vec<ListItem> = self.items.filtered.clone().unwrap().into_iter().map(|branch_info| {
                let branch_and_padding = branch_info.branch_name.pad_to_width(largest_string_len);
                let lines = vec![
                    Spans::from(format!("{}   changed: {}", branch_and_padding, branch_info.time_ago)),
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
}