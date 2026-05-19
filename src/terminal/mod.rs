pub mod ansi;
pub mod cell;
pub mod emulator;
pub mod events;
pub mod grid;
pub mod keys;
pub mod state;

pub use cell::{Cell, CellAttrs, Color};
pub use emulator::{Emulator, EmulatorState};
pub use events::{TerminalEvent, now_ms};
pub use grid::Grid;
pub use state::ProcessState;

pub type TerminalId = String;

pub const SIZE_80X24: (u16, u16) = (24, 80);
pub const SIZE_120X40: (u16, u16) = (40, 120);
pub const SIZE_160X40: (u16, u16) = (40, 160);
pub const SIZE_200X50: (u16, u16) = (50, 200);

pub fn parse_size(s: &str) -> anyhow::Result<(u16, u16)> {
    match s {
        "80x24" => Ok(SIZE_80X24),
        "120x40" => Ok(SIZE_120X40),
        "160x40" => Ok(SIZE_160X40),
        "200x50" => Ok(SIZE_200X50),
        other => {
            let parts: Vec<&str> = other.split('x').collect();
            if parts.len() == 2 {
                let cols: u16 = parts[0].parse()?;
                let rows: u16 = parts[1].parse()?;
                Ok((rows, cols))
            } else {
                anyhow::bail!("invalid size '{other}'; expected COLSxROWS, e.g. 80x24");
            }
        }
    }
}
