use clap::Parser;
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use githist::agent_store::{self, AgentStore};
use githist::git::branching::{Config, Repo};
use githist::ui::gui::{restore_terminal, setup_terminal};
use githist::{App, AppExit};
use std::fs::OpenOptions;
use std::io::{self, Write};
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

    let mut terminal = match setup_terminal() {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("couldn't open /dev/tty for the UI: {error}");
            return ExitCode::FAILURE;
        }
    };

    // Install panic hook that restores the terminal before printing the panic.
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        if let Ok(mut tty) = OpenOptions::new().write(true).open("/dev/tty") {
            let _ = execute!(tty, LeaveAlternateScreen);
        }
        original_hook(panic_info);
    }));

    let repo_id = repo.repo_id();
    let repo_cwd = repo.workdir_path();
    let agent_store = match AgentStore::load(&agent_store::store_path(&repo_id)) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("warning: couldn't load agent link store: {error}");
            AgentStore::default()
        }
    };

    let mut app = App::new(branches, agent_store, repo_id, repo_cwd);
    app.select_first_item_if_none();
    let exit = match app.run_app(&config, &mut repo, &mut terminal) {
        Ok(exit) => exit,
        Err(error) => {
            eprintln!("{error:?}");
            let _ = restore_terminal(&mut terminal);
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = restore_terminal(&mut terminal) {
        eprintln!("couldn't restore terminal: {error}");
        return ExitCode::FAILURE;
    }

    match exit {
        AppExit::Quit => ExitCode::SUCCESS,
        AppExit::Farewell(message) => {
            println!("{message}");
            ExitCode::SUCCESS
        }
        AppExit::WorktreePath(path) => {
            println!("{path}");
            let _ = io::stdout().flush();
            ExitCode::SUCCESS
        }
    }
}
