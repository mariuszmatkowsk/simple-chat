mod screen_state;

use crossterm::event::{self, poll, read, Event, KeyEventKind, KeyModifiers, KeyCode};
use crossterm::{execute, QueueableCommand};
use crossterm::{terminal, cursor};
use crossterm::style::Print;
use screen_state::ScreenState;
use std::io::{self, stdout, Write};
use std::time::Duration;

fn main() -> io::Result<()>  {
    let _screen_state = ScreenState::enable();
    let mut stdout = stdout();

    let mut quit = false;

    let mut prompt = String::new();
    let mut chat = Vec::<String>::new();

    execute!(stdout, terminal::Clear(terminal::ClearType::All))?;

    let (mut w, mut h) = terminal::size()?;

    while !quit {
       if poll(Duration::ZERO)? {
           match read()? {
               Event::Key(key_event) if key_event.kind == KeyEventKind::Press => { 
                    match key_event.code {
                        KeyCode::Char(ch) => {
                           if key_event.modifiers == KeyModifiers::CONTROL && ch == 'c' {
                               quit = true;
                           } else {
                               prompt.push(ch);
                           }
                       },
                       KeyCode::Enter => {
                           chat.push(prompt.clone());
                           prompt.clear();
                       },
                       KeyCode::Backspace => {
                           prompt.pop();
                       }
                       _ => (),
                   }
               },
               // hande other events
               _ => (),
           }
        }

        execute!(stdout, terminal::Clear(terminal::ClearType::All))?;

        for (dy, line) in chat.iter().enumerate() {
           stdout.queue(cursor::MoveTo(0, dy as u16))?;
           stdout.queue(Print(line))?;
        }

        if h.checked_sub(2).is_some() {
            stdout.queue(cursor::MoveTo(0, h-2))?;
            stdout.queue(Print("═".repeat(w.into())))?;
        }

        if h.checked_sub(1).is_some() {
            stdout.queue(cursor::MoveTo(0, h-1))?;
            stdout.queue(Print(prompt.clone()))?;
        }

        stdout.flush()?;
        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
