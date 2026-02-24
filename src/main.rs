use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use githist::git::branching::{Config, Repo};
use githist::ui::gui::{restore_terminal, setup_terminal};
use githist::App;
use std::error::Error;
use std::io;
use std::panic;

fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::parse();

    let repo = match Repo::open(&config) {
        Ok(repo) => repo,
        Err(error) => {
            eprintln!("{error:?}");
            return Ok(());
        }
    };

    match repo.get_branch_names() {
        Ok(result) => {
            let mut terminal = setup_terminal();

            // Install panic hook that restores the terminal before printing the panic.
            let original_hook = panic::take_hook();
            panic::set_hook(Box::new(move |panic_info| {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                original_hook(panic_info);
            }));

            let mut app = App::new(result);
            app.select_first_item_if_none();
            let res = app.run_app(&config, &repo, &mut terminal);
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
