use crossterm::{
    cursor,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    QueueableCommand,
};

use std::io::{self, Write};

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

pub struct Patch {
    cell: Cell,
    x: usize,
    y: usize,
}

pub struct TerminalBuffer {
    cells: Vec<Cell>,
    width: usize,
    height: usize,
}

impl TerminalBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let cells = vec![Cell::default(); width * height];
        Self {
            cells,
            width,
            height,
        }
    }

    pub fn put_cell(&mut self, ch: char, x: usize, y: usize, fg: Color, bg: Color) {
        let start_index = y * self.width + x;
        if let Some(cell) = self.cells.get_mut(start_index) {
            *cell = Cell { ch, fg, bg };
        }
    }

    pub fn put_cells(&mut self, chs: &[char], x: usize, y: usize, fg: Color, bg: Color) {
        let start_index = y * self.width + x;
        for (offset, ch) in chs.iter().enumerate() {
            if let Some(cell) = self.cells.get_mut(start_index + offset) {
                *cell = Cell { ch: *ch, fg, bg };
            } else {
                break;
            }
        }
    }

    pub fn diff(&mut self, other: &Self) -> Vec<Patch> {
        assert!(self.width == other.width && self.height == other.height);

        self.cells
            .iter()
            .zip(other.cells.iter())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, _))| {
                let x = i % self.width;
                let y = i / self.width;
                Patch {
                    cell: a.clone(),
                    x,
                    y,
                }
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
    }

    pub fn flush(&self, qc: &mut impl Write) -> io::Result<()> {
        let mut curr_fg_color = Color::White;
        let mut curr_bg_color = Color::Black;
        qc.queue(Clear(ClearType::All))?;
        qc.queue(SetForegroundColor(curr_fg_color))?;
        qc.queue(SetBackgroundColor(curr_bg_color))?;
        qc.queue(cursor::MoveTo(0, 0))?;
        for Cell { ch, fg, bg } in self.cells.iter() {
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

pub fn apply_patches(qc: &mut impl QueueableCommand, patches: &[Patch]) -> io::Result<()> {
    let mut fg_curr = Color::White;
    let mut bg_curr = Color::Black;
    let mut x_prev = 0;
    let mut y_prev = 0;
    qc.queue(SetForegroundColor(fg_curr))?;
    qc.queue(SetBackgroundColor(bg_curr))?;
    for Patch {
        cell: Cell { ch, fg, bg },
        x,
        y,
    } in patches
    {
        if !(y_prev == *y && x_prev + 1 == *x) {
            qc.queue(cursor::MoveTo(*x as u16, *y as u16))?;
        }
        x_prev = *x;
        y_prev = *y;
        if fg_curr != *fg {
            fg_curr = *fg;
            qc.queue(SetForegroundColor(fg_curr))?;
        }
        if bg_curr != *bg {
            bg_curr = *bg;
            qc.queue(SetBackgroundColor(bg_curr))?;
        }
        qc.queue(Print(ch))?;
    }
    Ok(())
}
