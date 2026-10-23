//! Minimal VT screen: grid of styled cells + scrollback.
//! Enough for shells and agent CLIs: SGR colors, cursor moves, erase,
//! insert/delete lines, alternate screen. Not a full terminal emulator.
use ratatui::style::{Color, Modifier, Style};
use std::collections::VecDeque;
use vte::{Params, Parser, Perform};

#[derive(Clone, Copy, PartialEq)]
pub struct Cell {
    pub ch: char,
    pub style: Style,
}
impl Default for Cell {
    fn default() -> Self {
        Self { ch: ' ', style: Style::default() }
    }
}

pub struct Screen {
    pub cols: usize,
    pub rows: usize,
    grid: Vec<Vec<Cell>>,
    alt: Vec<Vec<Cell>>,
    on_alt: bool,
    cx: usize,
    cy: usize,
    style: Style,
    pub scrollback: VecDeque<Vec<Cell>>,
}

const MAX_SCROLLBACK: usize = 2000;

impl Screen {
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.max(2);
        let rows = rows.max(2);
        Self {
            cols,
            rows,
            grid: vec![vec![Cell::default(); cols]; rows],
            alt: vec![vec![Cell::default(); cols]; rows],
            on_alt: false,
            cx: 0,
            cy: 0,
            style: Style::default(),
            scrollback: VecDeque::new(),
        }
    }
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(2);
        let rows = rows.max(2);
        for g in [&mut self.grid, &mut self.alt] {
            g.resize(rows, vec![Cell::default(); cols]);
            for row in g.iter_mut() {
                row.resize(cols, Cell::default());
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.cx = self.cx.min(cols - 1);
        self.cy = self.cy.min(rows - 1);
    }
    /// Line at display offset: 0..scrollback.len() is history, then grid.
    pub fn display_line(&self, offset_from_bottom: usize, visible_rows: usize) -> Vec<&Vec<Cell>> {
        let total = self.scrollback.len() + self.rows;
        let end = total.saturating_sub(offset_from_bottom);
        let start = end.saturating_sub(visible_rows);
        (start..end)
            .map(|i| {
                if i < self.scrollback.len() {
                    &self.scrollback[i]
                } else {
                    &self.grid[i - self.scrollback.len()]
                }
            })
            .collect()
    }
    pub fn cursor(&self) -> (usize, usize) { (self.cx, self.cy) }
    fn scroll_up(&mut self, n: usize) {
        for _ in 0..n {
            let top = self.grid.remove(0);
            if !self.on_alt {
                self.scrollback.push_back(top);
                if self.scrollback.len() > MAX_SCROLLBACK {
                    self.scrollback.pop_front();
                }
            }
            self.grid.push(vec![Cell::default(); self.cols]);
        }
    }
    fn put(&mut self, ch: char) {
        if self.cx >= self.cols {
            self.cx = 0;
            self.line_feed();
        }
        let cell = Cell { ch, style: self.style };
        self.grid[self.cy][self.cx] = cell;
        self.cx += 1;
    }
    fn line_feed(&mut self) {
        if self.cy + 1 >= self.rows {
            self.scroll_up(1);
        } else {
            self.cy += 1;
        }
    }
}

fn color_ansi(code: u32, bright: bool) -> Color {
    match (code, bright) {
        (0, false) => Color::Black,
        (1, false) => Color::Red,
        (2, false) => Color::Green,
        (3, false) => Color::Yellow,
        (4, false) => Color::Blue,
        (5, false) => Color::Magenta,
        (6, false) => Color::Cyan,
        (7, false) => Color::Gray,
        (0, true) => Color::DarkGray,
        (1, true) => Color::LightRed,
        (2, true) => Color::LightGreen,
        (3, true) => Color::LightYellow,
        (4, true) => Color::LightBlue,
        (5, true) => Color::LightMagenta,
        (6, true) => Color::LightCyan,
        (_, _) => Color::White,
    }
}

impl Perform for Screen {
    fn print(&mut self, c: char) {
        self.put(c);
    }
    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' | 0x0b | 0x0c => {
                self.line_feed();
            }
            b'\r' => self.cx = 0,
            0x08 => self.cx = self.cx.saturating_sub(1),
            b'\t' => {
                let next = ((self.cx / 8) + 1) * 8;
                self.cx = next.min(self.cols - 1);
            }
            _ => {}
        }
    }
    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        let p: Vec<u32> = params
            .iter()
            .map(|s| s.first().copied().unwrap_or(0) as u32)
            .collect();
        let n = |i: usize| p.get(i).copied().unwrap_or(0);
        let count = |i: usize| match n(i) { 0 => 1, v => v } as usize;
        match action {
            'A' => self.cy = self.cy.saturating_sub(count(0)),
            'B' => self.cy = (self.cy + count(0)).min(self.rows - 1),
            'C' => self.cx = (self.cx + count(0)).min(self.cols - 1),
            'D' => self.cx = self.cx.saturating_sub(count(0)),
            'G' | '`' => self.cx = count(0).saturating_sub(1).min(self.cols - 1),
            'H' | 'f' => {
                self.cy = count(0).saturating_sub(1).min(self.rows - 1);
                self.cx = count(1).saturating_sub(1).min(self.cols - 1);
            }
            'd' => self.cy = count(0).saturating_sub(1).min(self.rows - 1),
            'J' => match n(0) {
                0 => {
                    for x in self.cx..self.cols {
                        self.grid[self.cy][x] = Cell::default();
                    }
                    for row in self.grid.iter_mut().skip(self.cy + 1) {
                        row.fill(Cell::default());
                    }
                }
                1 => {
                    for row in self.grid.iter_mut().take(self.cy) {
                        row.fill(Cell::default());
                    }
                    for x in 0..=self.cx.min(self.cols - 1) {
                        self.grid[self.cy][x] = Cell::default();
                    }
                }
                _ => {
                    for row in self.grid.iter_mut() {
                        row.fill(Cell::default());
                    }
                }
            },
            'K' => match n(0) {
                0 => {
                    for x in self.cx..self.cols {
                        self.grid[self.cy][x] = Cell::default();
                    }
                }
                1 => {
                    for x in 0..=self.cx.min(self.cols - 1) {
                        self.grid[self.cy][x] = Cell::default();
                    }
                }
                _ => self.grid[self.cy].fill(Cell::default()),
            },
            'L' => {
                for _ in 0..count(0) {
                    self.grid.insert(self.cy, vec![Cell::default(); self.cols]);
                    self.grid.truncate(self.rows);
                }
            }
            'M' => {
                for _ in 0..count(0) {
                    if self.cy < self.grid.len() {
                        self.grid.remove(self.cy);
                    }
                    self.grid.push(vec![Cell::default(); self.cols]);
                }
            }
            'P' => {
                for _ in 0..count(0).min(self.cols) {
                    if self.cx < self.cols {
                        self.grid[self.cy].remove(self.cx);
                        self.grid[self.cy].push(Cell::default());
                    }
                }
            }
            '@' => {
                for _ in 0..count(0).min(self.cols) {
                    if self.cx < self.cols {
                        self.grid[self.cy].insert(self.cx, Cell::default());
                        self.grid[self.cy].truncate(self.cols);
                    }
                }
            }
            'S' => self.scroll_up(count(0)),
            'm' => {
                if p.is_empty() {
                    self.style = Style::default();
                }
                let mut i = 0;
                while i < p.len() {
                    match p[i] {
                        0 => self.style = Style::default(),
                        1 => self.style = self.style.add_modifier(Modifier::BOLD),
                        2 => self.style = self.style.add_modifier(Modifier::DIM),
                        3 => self.style = self.style.add_modifier(Modifier::ITALIC),
                        4 => self.style = self.style.add_modifier(Modifier::UNDERLINED),
                        7 => self.style = self.style.add_modifier(Modifier::REVERSED),
                        22 => self.style = self.style.remove_modifier(Modifier::BOLD | Modifier::DIM),
                        23 => self.style = self.style.remove_modifier(Modifier::ITALIC),
                        24 => self.style = self.style.remove_modifier(Modifier::UNDERLINED),
                        27 => self.style = self.style.remove_modifier(Modifier::REVERSED),
                        30..=37 => self.style = self.style.fg(color_ansi(p[i] - 30, false)),
                        39 => self.style = self.style.fg(Color::Reset),
                        40..=47 => self.style = self.style.bg(color_ansi(p[i] - 40, false)),
                        49 => self.style = self.style.bg(Color::Reset),
                        90..=97 => self.style = self.style.fg(color_ansi(p[i] - 90, true)),
                        100..=107 => self.style = self.style.bg(color_ansi(p[i] - 100, true)),
                        38 | 48 => {
                            let fg = p[i] == 38;
                            if p.get(i + 1) == Some(&5) {
                                if let Some(v) = p.get(i + 2) {
                                    let c = Color::Indexed(*v as u8);
                                    self.style = if fg { self.style.fg(c) } else { self.style.bg(c) };
                                }
                                i += 2;
                            } else if p.get(i + 1) == Some(&2) {
                                if let (Some(r), Some(g), Some(b)) = (p.get(i + 2), p.get(i + 3), p.get(i + 4)) {
                                    let c = Color::Rgb(*r as u8, *g as u8, *b as u8);
                                    self.style = if fg { self.style.fg(c) } else { self.style.bg(c) };
                                }
                                i += 4;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            'h' | 'l' => {
                if params.iter().any(|s| s.first() == Some(&1049) || s.first() == Some(&47)) {
                    if action == 'h' && !self.on_alt {
                        std::mem::swap(&mut self.grid, &mut self.alt);
                        self.on_alt = true;
                    } else if action == 'l' && self.on_alt {
                        std::mem::swap(&mut self.grid, &mut self.alt);
                        self.on_alt = false;
                    }
                }
            }
            _ => {}
        }
    }
}

pub struct VtParser {
    parser: Parser,
}
impl VtParser {
    pub fn new() -> Self {
        Self { parser: Parser::new() }
    }
    pub fn feed(&mut self, screen: &mut Screen, bytes: &[u8]) {
        for byte in bytes {
            self.parser.advance(screen, *byte);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn feed(s: &mut Screen, bytes: &[u8]) {
        VtParser::new().feed(s, bytes);
    }
    #[test]
    fn text_cursor_and_sgr() {
        let mut s = Screen::new(10, 3);
        feed(&mut s, b"hi\x1b[31m!\x1b[0m\r\nbye");
        assert_eq!(s.grid[0][0].ch, 'h');
        assert_eq!(s.grid[0][2].ch, '!');
        assert_eq!(s.grid[0][2].style.fg, Some(Color::Red));
        assert_eq!(s.grid[1][0].ch, 'b');
    }
    #[test]
    fn scroll_pushes_scrollback() {
        let mut s = Screen::new(5, 2);
        feed(&mut s, b"a\r\nb\r\nc");
        assert_eq!(s.scrollback.len(), 1);
        assert_eq!(s.grid[1][0].ch, 'c');
        let lines = s.display_line(1, 2);
        assert_eq!(lines[0][0].ch, 'a');
    }
    #[test]
    fn erase_and_resize() {
        let mut s = Screen::new(5, 2);
        feed(&mut s, b"hello\x1b[2J");
        assert!(s.grid.iter().flatten().all(|c| c.ch == ' '));
        s.resize(3, 3);
        assert_eq!((s.cols, s.rows), (3, 3));
    }
}
