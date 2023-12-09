use std::io::{self, stdout};
use crossterm:: {execute, terminal};

pub struct ScreenState;

impl ScreenState {
    pub fn enable() -> io::Result<Self> {
        execute!(stdout(), terminal::EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for ScreenState {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode().map_err(|err| {
            eprintln!("ERROR: could not disable raw mode: {err}");
        });
        let _ = execute!(stdout(), terminal::LeaveAlternateScreen).map_err(|err| {
            eprintln!("ERROR: could not leave alternate screen: {err}");
        });
    }
}

