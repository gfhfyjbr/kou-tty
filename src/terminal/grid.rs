use std::collections::{HashSet, VecDeque};

use super::cell::{Cell, CellAttrs, Color};

const MAX_SCROLLBACK: usize = 5000;

#[derive(Clone, Debug)]
pub struct Grid {
    pub rows: u16,
    pub cols: u16,
    pub cells: Vec<Vec<Cell>>,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    pub current_fg: Color,
    pub current_bg: Color,
    pub current_attrs: CellAttrs,
    pub dirty_rows: HashSet<u16>,
    pub scrollback: VecDeque<Vec<Cell>>,
    pub seq: u64,
    pub saved_cursor: Option<(u16, u16)>,
}

impl Grid {
    pub fn new(rows: u16, cols: u16) -> Self {
        let cells = (0..rows)
            .map(|_| vec![Cell::empty(); cols as usize])
            .collect();
        Self {
            rows,
            cols,
            cells,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            current_fg: Color::Default,
            current_bg: Color::Default,
            current_attrs: CellAttrs::default(),
            dirty_rows: (0..rows).collect(),
            scrollback: VecDeque::with_capacity(MAX_SCROLLBACK),
            seq: 0,
            saved_cursor: None,
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        let mut cells: Vec<Vec<Cell>> = (0..rows)
            .map(|_| vec![Cell::empty(); cols as usize])
            .collect();
        for (r, row) in self.cells.iter().enumerate().take(rows as usize) {
            for (c, cell) in row.iter().enumerate().take(cols as usize) {
                cells[r][c] = *cell;
            }
        }
        self.cells = cells;
        self.rows = rows;
        self.cols = cols;
        self.cursor_row = self.cursor_row.min(rows.saturating_sub(1));
        self.cursor_col = self.cursor_col.min(cols.saturating_sub(1));
        self.mark_all_dirty();
        self.seq = self.seq.wrapping_add(1);
    }

    pub fn mark_dirty(&mut self, row: u16) {
        if row < self.rows {
            self.dirty_rows.insert(row);
            self.seq = self.seq.wrapping_add(1);
        }
    }

    pub fn mark_all_dirty(&mut self) {
        self.dirty_rows = (0..self.rows).collect();
        self.seq = self.seq.wrapping_add(1);
    }

    pub fn take_dirty(&mut self) -> Vec<u16> {
        let mut v: Vec<u16> = self.dirty_rows.drain().collect();
        v.sort_unstable();
        v
    }

    pub fn put_char(&mut self, ch: char) {
        if self.cursor_col >= self.cols {
            self.line_feed();
            self.cursor_col = 0;
        }
        let row = self.cursor_row as usize;
        let col = self.cursor_col as usize;
        if row < self.cells.len() && col < self.cells[row].len() {
            self.cells[row][col] = Cell {
                ch,
                fg: self.current_fg,
                bg: self.current_bg,
                attrs: self.current_attrs,
            };
            self.mark_dirty(self.cursor_row);
        }
        self.cursor_col = self.cursor_col.saturating_add(1);
    }

    pub fn line_feed(&mut self) {
        if self.cursor_row + 1 >= self.rows {
            // scroll up
            let first = self.cells.remove(0);
            if self.scrollback.len() == MAX_SCROLLBACK {
                self.scrollback.pop_front();
            }
            self.scrollback.push_back(first);
            self.cells.push(vec![Cell::empty(); self.cols as usize]);
            self.mark_all_dirty();
        } else {
            self.cursor_row += 1;
        }
    }

    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn horizontal_tab(&mut self) {
        let next = ((self.cursor_col / 8) + 1) * 8;
        self.cursor_col = next.min(self.cols.saturating_sub(1));
    }

    pub fn cursor_move(&mut self, row: u16, col: u16) {
        self.cursor_row = row.min(self.rows.saturating_sub(1));
        self.cursor_col = col.min(self.cols.saturating_sub(1));
    }

    pub fn cursor_up(&mut self, n: u16) {
        self.cursor_row = self.cursor_row.saturating_sub(n.max(1));
    }

    pub fn cursor_down(&mut self, n: u16) {
        self.cursor_row = self
            .cursor_row
            .saturating_add(n.max(1))
            .min(self.rows.saturating_sub(1));
    }

    pub fn cursor_forward(&mut self, n: u16) {
        self.cursor_col = self
            .cursor_col
            .saturating_add(n.max(1))
            .min(self.cols.saturating_sub(1));
    }

    pub fn cursor_back(&mut self, n: u16) {
        self.cursor_col = self.cursor_col.saturating_sub(n.max(1));
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some((self.cursor_row, self.cursor_col));
    }

    pub fn restore_cursor(&mut self) {
        if let Some((r, c)) = self.saved_cursor {
            self.cursor_row = r.min(self.rows.saturating_sub(1));
            self.cursor_col = c.min(self.cols.saturating_sub(1));
        }
    }

    pub fn erase_in_display(&mut self, mode: u16) {
        match mode {
            0 => {
                // from cursor to end of screen
                let cur_row = self.cursor_row as usize;
                let cur_col = self.cursor_col as usize;
                if let Some(row) = self.cells.get_mut(cur_row) {
                    for cell in row.iter_mut().skip(cur_col) {
                        *cell = Cell::empty();
                    }
                }
                for r in (cur_row + 1)..self.cells.len() {
                    for cell in &mut self.cells[r] {
                        *cell = Cell::empty();
                    }
                }
            }
            1 => {
                // start to cursor
                let cur_row = self.cursor_row as usize;
                let cur_col = self.cursor_col as usize;
                for r in 0..cur_row {
                    for cell in &mut self.cells[r] {
                        *cell = Cell::empty();
                    }
                }
                if let Some(row) = self.cells.get_mut(cur_row) {
                    for cell in row.iter_mut().take(cur_col + 1) {
                        *cell = Cell::empty();
                    }
                }
            }
            _ => {
                for row in &mut self.cells {
                    for cell in row.iter_mut() {
                        *cell = Cell::empty();
                    }
                }
            }
        }
        self.mark_all_dirty();
    }

    pub fn erase_in_line(&mut self, mode: u16) {
        let cur_row = self.cursor_row as usize;
        let cur_col = self.cursor_col as usize;
        let Some(row) = self.cells.get_mut(cur_row) else {
            return;
        };
        match mode {
            0 => {
                for cell in row.iter_mut().skip(cur_col) {
                    *cell = Cell::empty();
                }
            }
            1 => {
                for cell in row.iter_mut().take(cur_col + 1) {
                    *cell = Cell::empty();
                }
            }
            _ => {
                for cell in row.iter_mut() {
                    *cell = Cell::empty();
                }
            }
        }
        self.mark_dirty(self.cursor_row);
    }

    pub fn row_text(&self, row: u16) -> String {
        let Some(line) = self.cells.get(row as usize) else {
            return String::new();
        };
        let mut s: String = line.iter().map(|c| c.ch).collect();
        let trimmed_len = s.trim_end().len();
        s.truncate(trimmed_len);
        s
    }

    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for row in 0..self.rows {
            out.push_str(&self.row_text(row));
            out.push('\n');
        }
        out
    }
}
