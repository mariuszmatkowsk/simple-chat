mod screen_state;

use crossterm::QueueableCommand;
use crossterm::event::{ poll, read, Event, KeyEventKind, KeyModifiers, KeyCode };
use crossterm::style::{ Print, Color, SetForegroundColor, SetBackgroundColor };
use crossterm::{ terminal, cursor };
use screen_state::ScreenState;
use std::io::{ self, stdout, Write };
use std::time::Duration;

#[derive(Clone, PartialEq)]
struct Cell {
    ch: char,
    fg: Color,
    bg: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::White,
            bg: Color::Black,
        }
    }
}

struct Patch {
    cell: Cell,
    x: usize,
    y: usize,
}

struct Buffer {
    cells: Vec<Cell>,
    width: usize,
    height: usize,
}

impl Buffer {
    fn new(width: usize, height: usize) -> Self {
        let cells = vec![Cell::default(); width*height];
        Self { cells, width, height }
    }

    fn put_cell(&mut self, ch: char, x: usize, y: usize, fg: Color, bg: Color) {
        let start_index = y * self.width + x;
        if let Some(cell) = self.cells.get_mut(start_index) {
            *cell = Cell { ch, fg, bg };
        }
    }

    fn put_cells(&mut self, chs: &[char], x: usize, y: usize, fg: Color, bg: Color) {
        let start_index = y * self.width + x;
        for (offset, ch) in chs.iter().enumerate() {
            if let Some(cell) = self.cells.get_mut(start_index + offset) {
                *cell = Cell { ch: *ch, fg, bg };    
            } else {
                break;
            }
        }
    }

    fn diff(&mut self, other: &Self) -> Vec<Patch> {
        assert!(self.width == other.width && self.height == other.height);

        self.cells
            .iter()
            .zip(other.cells.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, _))| {
                let x = i%self.width;
                let y = i/self.width;
                Patch {cell: a.clone(), x, y}
            })
            .collect()
    }

    fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    fn flush(&self, qc: &mut impl Write) -> io::Result<()> {
        let mut curr_fg_color = Color::White;
        let mut curr_bg_color = Color::Black;
        qc.queue(terminal::Clear(terminal::ClearType::All))?;
        qc.queue(SetForegroundColor(curr_fg_color))?;
        qc.queue(SetBackgroundColor(curr_bg_color))?;
        qc.queue(cursor::MoveTo(0, 0))?;
        for Cell { ch, fg, bg }in self.cells.iter() {
            if curr_fg_color != *fg {
                curr_fg_color = *fg;
                qc.queue(SetForegroundColor(curr_fg_color))?;
            }

            if curr_bg_color != *bg {
                curr_bg_color = *bg;
                qc.queue(SetBackgroundColor(curr_bg_color))?;
            }

            qc.queue(Print(ch))?;
        }
        qc.flush()?;
        Ok(())
    }
}

fn apply_patches(qc: &mut impl Write, patches: &Vec<Patch>) -> io::Result<()> {
    for Patch { cell, x, y } in patches.iter() {
        qc.queue(cursor::MoveTo(*x as u16, *y as u16))?;
        qc.queue(SetForegroundColor(cell.fg))?;
        qc.queue(SetBackgroundColor(cell.bg))?;
        qc.queue(Print(cell.ch))?;
    }

    Ok(())
}

#[derive(Default)]
struct Prompt {
    data: String,
    cursor: usize,
}

impl Prompt {
    fn insert(&mut self, ch: char) {
        self.cursor += 1;
        self.data.push(ch);
    }

    fn backspace(&mut self) {
        self.cursor = if self.cursor > 1 { self.cursor - 1 } else { 0 };
        self.data.pop();
    }

    fn clear(&mut self) {
        self.cursor = 0;
        self.data.clear();
    }

    fn render(&self, buffer: &mut Buffer, x: usize, y: usize, w: usize) {
        let chars: Vec<_> = self.data.chars().collect();
        buffer.put_cells(&chars, x, y, Color::White, Color::Black);

        for pos_x in chars.len()..w {
            buffer.put_cell(' ', pos_x, y, Color::White, Color::Black);
        }
    }

    fn sync_cursor_with_terminal(&self, qc: &mut impl Write, x: usize, y: usize, w: usize) -> io::Result<()> {
        let cursor_x_pos = std::cmp::min(x + self.cursor, w);
        qc.queue(cursor::MoveTo(cursor_x_pos as u16, y as u16))?;
        Ok(())
    }

    fn get(&self) -> String {
        self.data.clone()
    }
}

struct ChatLog {
    data: Vec<String>,
}

impl Default for ChatLog {
    fn default() -> Self {
        Self { data: Vec::<String>::default() }
    }
}

impl ChatLog {
    fn insert(&mut self, line: String) {
        self.data.push(line)
    }

    fn render(&self, buffer: &mut Buffer, x: usize, y: usize) {
        for (dy, line) in self.data.iter().enumerate() {
            let line_chars: Vec<_> = line.chars().collect();
            buffer.put_cells(&line_chars, x, y+dy, Color::White, Color::Black);
        }
    }
}

fn status_bar(buffer: &mut Buffer, label: &str, x: usize, y: usize, w: usize) {
    let label_chars: Vec<_> = label.chars().collect();
    let n = std::cmp::min(label_chars.len(), w);

    buffer.put_cells(&label_chars[..n], x, y, Color::Black, Color::White);

    for pos_x in label_chars.len()..w {
        buffer.put_cell(' ', pos_x, y, Color::Black, Color::White);
    }
}

fn main() -> io::Result<()>  {
    let _screen_state = ScreenState::enable();
    let mut stdout = stdout();

    let mut chat = ChatLog::default();
    let mut prompt = Prompt::default();

    let (w, h) = terminal::size()?;

    let mut screen_buffer = Buffer::new(w as usize, h as usize);
    let mut prev_screen_buffer = Buffer::new(w as usize, h as usize);

    let mut quit = false;
    

    prev_screen_buffer.flush(&mut stdout)?;
    while !quit {
       if poll(Duration::ZERO)? {
           match read()? {
               Event::Key(key_event) if key_event.kind == KeyEventKind::Press => { 
                    match key_event.code {
                        KeyCode::Char(ch) => {
                           if key_event.modifiers == KeyModifiers::CONTROL && ch == 'c' {
                               quit = true;
                           } else {
                               prompt.insert(ch);
                           }
                       },
                       KeyCode::Enter => {
                           chat.insert(prompt.get());
                           prompt.clear();
                       },
                       KeyCode::Backspace => {
                           prompt.backspace();
                       }
                       _ => (),
                   }
               },
               // hande other events
               _ => (),
           }
        }

        screen_buffer.clear();

        status_bar(&mut screen_buffer, "simple-chat", 0, 0, w.into());

        chat.render(&mut screen_buffer, 0, 1);

        if h.checked_sub(2).is_some() {
            status_bar(&mut screen_buffer, "Online", 0, (h - 2).into(), w.into())
        }

        if h.checked_sub(1).is_some() {
            prompt.render(&mut screen_buffer, 0, (h - 1).into() , w.into());
            prompt.sync_cursor_with_terminal(&mut stdout, 0, (h - 1).into(), w.into())?;
        }

        let patches = screen_buffer.diff(&prev_screen_buffer);
        
        apply_patches(&mut stdout, &patches)?;

        stdout.flush()?;

        std::mem::swap(&mut screen_buffer, &mut prev_screen_buffer);

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
