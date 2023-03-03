use std::{env, io};
use std::error::Error;
use std::io::{Stdout, stdout};
use std::time::{Duration, Instant};
use crossterm::{
    event::{self, Event, KeyCode},
};
use tui::backend::{Backend, CrosstermBackend};
use tui::{Frame, Terminal};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Modifier, Style};
use tui::text::{Span, Spans};
use tui::widgets::{Block, Borders, List, ListItem};
use githist::App;
use githist::git_fns::git_fns::{Config, get_branch_names};
use githist::ui::app_ui::{restore_terminal, setup_terminal};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let config = Config::new(args);
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    match get_branch_names(&config) {
        Ok(result) => {
            setup_terminal();
            let vec = result;
            let mut app = App::new(vec);
            let res = app.run_app(&config, &mut terminal);
            if let Err(err) = res {
                println!("{:?}", err);
            }
            restore_terminal(&mut terminal).expect("couldn't restore!");
        }
        Err(error) => {
            println!("{:?}", error);
        }
    }
    Ok(())
}