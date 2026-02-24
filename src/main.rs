use githist::git::branching::{get_branch_names, Config};
use githist::ui::gui::{restore_terminal, setup_terminal};
use githist::App;
use std::error::Error;
use std::env;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    let config = Config::new(&args);

    match get_branch_names(&config) {
        Ok(result) => {
            let mut terminal = setup_terminal();
            let mut app = App::new(result);
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
