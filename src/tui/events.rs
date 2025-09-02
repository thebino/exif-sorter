use crossterm::event::{self, KeyEventKind};

use super::app::App;
use std::io::Result;

pub fn handle_events(app: &mut App) -> Result<()> {
    if event::poll(std::time::Duration::from_millis(16))? {
        if let event::Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                app.handle_event(key);
            }
        }
    }

    Ok(())
}
