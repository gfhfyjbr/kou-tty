use vte::{Params, Perform};

use super::cell::{CellAttrs, Color};
use super::events::TerminalEvent;
use super::grid::Grid;

pub struct AnsiHandler<'a> {
    pub grid: &'a mut Grid,
    pub events: &'a mut Vec<TerminalEvent>,
}

impl<'a> AnsiHandler<'a> {
    pub fn new(grid: &'a mut Grid, events: &'a mut Vec<TerminalEvent>) -> Self {
        Self { grid, events }
    }
}

fn first_param(params: &Params, default: u16) -> u16 {
    params
        .iter()
        .next()
        .and_then(|p| p.first().copied())
        .unwrap_or(default)
        .max(default)
}

fn collect_params(params: &Params) -> Vec<u16> {
    params.iter().flat_map(|p| p.iter().copied()).collect()
}

impl<'a> Perform for AnsiHandler<'a> {
    fn print(&mut self, c: char) {
        self.grid.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.events.push(TerminalEvent::Bell),
            0x08 => self.grid.backspace(),
            0x09 => self.grid.horizontal_tab(),
            0x0A | 0x0B | 0x0C => self.grid.line_feed(),
            0x0D => self.grid.carriage_return(),
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, c: char) {
        match c {
            'A' => self.grid.cursor_up(first_param(params, 1)),
            'B' => self.grid.cursor_down(first_param(params, 1)),
            'C' => self.grid.cursor_forward(first_param(params, 1)),
            'D' => self.grid.cursor_back(first_param(params, 1)),
            'H' | 'f' => {
                let mut iter = params.iter();
                let row = iter.next().and_then(|p| p.first().copied()).unwrap_or(1);
                let col = iter.next().and_then(|p| p.first().copied()).unwrap_or(1);
                self.grid
                    .cursor_move(row.saturating_sub(1), col.saturating_sub(1));
            }
            'J' => self.grid.erase_in_display(first_param(params, 0)),
            'K' => self.grid.erase_in_line(first_param(params, 0)),
            'm' => handle_sgr(self.grid, &collect_params(params)),
            's' => self.grid.save_cursor(),
            'u' => self.grid.restore_cursor(),
            'h' | 'l' => { /* mode set/reset — ignore */ }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, byte: u8) {
        match byte {
            b'7' => self.grid.save_cursor(),
            b'8' => self.grid.restore_cursor(),
            b'M' => {
                // Reverse Index
                if self.grid.cursor_row == 0 {
                    // scroll down — minimal impl: leave as is
                } else {
                    self.grid.cursor_row -= 1;
                }
            }
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {
        // window title etc — ignored
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _c: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}
}

fn handle_sgr(grid: &mut Grid, params: &[u16]) {
    if params.is_empty() {
        grid.current_fg = Color::Default;
        grid.current_bg = Color::Default;
        grid.current_attrs = CellAttrs::default();
        return;
    }
    let mut i = 0;
    while i < params.len() {
        let p = params[i];
        match p {
            0 => {
                grid.current_fg = Color::Default;
                grid.current_bg = Color::Default;
                grid.current_attrs = CellAttrs::default();
            }
            1 => grid.current_attrs.bold = true,
            3 => grid.current_attrs.italic = true,
            4 => grid.current_attrs.underline = true,
            7 => grid.current_attrs.inverse = true,
            9 => grid.current_attrs.strikethrough = true,
            22 => grid.current_attrs.bold = false,
            23 => grid.current_attrs.italic = false,
            24 => grid.current_attrs.underline = false,
            27 => grid.current_attrs.inverse = false,
            29 => grid.current_attrs.strikethrough = false,
            30..=37 => grid.current_fg = Color::Indexed((p - 30) as u8),
            38 => {
                if let Some(&mode) = params.get(i + 1) {
                    if mode == 5 {
                        if let Some(&idx) = params.get(i + 2) {
                            grid.current_fg = Color::Indexed(idx as u8);
                            i += 2;
                        }
                    } else if mode == 2 {
                        if let (Some(&r), Some(&g), Some(&b)) =
                            (params.get(i + 2), params.get(i + 3), params.get(i + 4))
                        {
                            grid.current_fg = Color::Rgb(r as u8, g as u8, b as u8);
                            i += 4;
                        }
                    }
                }
            }
            39 => grid.current_fg = Color::Default,
            40..=47 => grid.current_bg = Color::Indexed((p - 40) as u8),
            48 => {
                if let Some(&mode) = params.get(i + 1) {
                    if mode == 5 {
                        if let Some(&idx) = params.get(i + 2) {
                            grid.current_bg = Color::Indexed(idx as u8);
                            i += 2;
                        }
                    } else if mode == 2 {
                        if let (Some(&r), Some(&g), Some(&b)) =
                            (params.get(i + 2), params.get(i + 3), params.get(i + 4))
                        {
                            grid.current_bg = Color::Rgb(r as u8, g as u8, b as u8);
                            i += 4;
                        }
                    }
                }
            }
            49 => grid.current_bg = Color::Default,
            90..=97 => grid.current_fg = Color::Indexed((p - 90 + 8) as u8),
            100..=107 => grid.current_bg = Color::Indexed((p - 100 + 8) as u8),
            _ => {}
        }
        i += 1;
    }
}
