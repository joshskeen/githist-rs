pub mod app_ui {
    use std::io;
    use std::io::Stdout;
    use crossterm::execute;
    use tui::backend::CrosstermBackend;
    use tui::Terminal;
    use crossterm::{
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };

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


}