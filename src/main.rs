use std::{env, io};
use std::error::Error;
use tui::backend::{CrosstermBackend};
use tui::{Terminal};
use githist::App;
use githist::git::branching::{Config, get_branch_names};
use githist::ui::gui::{restore_terminal, setup_terminal};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let config = Config::new(&args);
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    match get_branch_names(&config) {
        Ok(result) => {
            setup_terminal();
            let vec = result;
            let mut app = App::new(vec);
            app.select_first_item_if_none();
            let res = app.run_app(&config, &mut terminal);
            if let Err(err) = res {
                eprintln!("{err:?}");
            }
            restore_terminal(&mut terminal).expect("couldn't restore!");
        }
        Err(error) => {
            eprintln!("{error:?}");
        }
    }
    Ok(())
}