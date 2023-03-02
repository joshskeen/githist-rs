use std::{env, io};
use std::error::Error;
use std::time::{Duration, Instant};
use githist::{App, BranchInfo, change_branch, Config, get_branch_names, NoSelectionError};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::backend::{Backend, CrosstermBackend};
use tui::{Frame, Terminal};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, List, ListItem};


fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let tick_rate = Duration::from_millis(250);

    let args: Vec<String> = env::args().collect();
    let path = if let Some(path) = &args.get(1) {
        path.to_string()
    } else {
        String::from(".")
    };
    let config = Config {
        repo_path: path
    };

    match get_branch_names(&config) {
        Ok(result) => {
            let vec = result;
            let app = App::new(vec);
            let res = run_app(&config, &mut terminal, app, tick_rate);
            // restore terminal
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;

            if let Err(err) = res {
                println!("{:?}", err)
            }
        }
        Err(_) => {}
    }
    Ok(())
}

fn run_app<B: Backend>(
    config: &Config,
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Enter => {
                        let branch_name = app.get_selected_branch_name();
                        match branch_name {
                            Ok(name) => {
                                change_branch(config, &name);
                            }
                            Err(_) => {
                                println!("no selection, nothing to do!")
                            }
                        }

                        return Ok(());
                    }
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Left => app.items.unselect(),
                    KeyCode::Down => app.items.next(),
                    KeyCode::Up => app.items.previous(),
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Create two chunks with equal horizontal screen space
    let chunks = Layout::default()
        .constraints([Constraint::Percentage(100)].as_ref())
        .direction(Direction::Horizontal)
        .split(f.size());

    let items: Vec<ListItem> = app
        .items
        .items
        .iter()
        .map(|branch_info| {
            let lines = vec![Spans::from(branch_info.branch_name.to_string())];
            ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
        })
        .collect();

    // Create a List from all list items and highlight the currently selected one
    let items = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("choose recent branch"))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // We can now render the item list
    f.render_stateful_widget(items, chunks[0], &mut app.items.state);
}

