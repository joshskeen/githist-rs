use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use githist::git::branching::{Config, Repo};
use githist::ui::gui::{restore_terminal, setup_terminal};
use githist::App;
use std::io;
use std::panic;
use std::process::ExitCode;

fn main() -> ExitCode {
    let config = Config::parse();

    let mut repo = match Repo::open(&config) {
        Ok(repo) => repo,
        Err(error) => {
            eprintln!("couldn't open repository: {}", error.message());
            return ExitCode::FAILURE;
        }
    };

    let branches = match repo.get_branch_names() {
        Ok(branches) => branches,
        Err(error) => {
            eprintln!("couldn't read branches: {}", error.message());
            return ExitCode::FAILURE;
        }
    };

    let mut terminal = setup_terminal();

    // Install panic hook that restores the terminal before printing the panic.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let mut app = App::new(branches);
    app.select_first_item_if_none();
    let result = app.run_app(&config, &mut repo, &mut terminal);
    restore_terminal(&mut terminal).expect("couldn't restore terminal!");
    match result {
        Ok(Some(message)) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:?}");
            ExitCode::FAILURE
        }
    }
}
