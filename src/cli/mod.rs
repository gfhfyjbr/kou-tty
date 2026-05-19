pub mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "kou-tty",
    version,
    about = "Headless in-memory terminal emulator CLI",
    long_about = "kou-tty is a headless terminal emulator. It spawns a PTY for each \
                  terminal, drains output through a VT/ANSI parser into an in-memory \
                  grid, and exposes 17 CLI subcommands plus a JSON stdin/stdout mode \
                  for AI agents."
)]
pub struct Cli {
    /// Path to daemon socket. Default: $XDG_RUNTIME_DIR/kou-tty-<uid>.sock or $TMPDIR equivalent.
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run the daemon in the foreground.
    Daemon,

    /// Read JSON-RPC requests from stdin, write responses to stdout.
    Json,

    /// Interactive REPL for manual debugging.
    Repl,

    /// Create a new terminal.
    Create {
        /// Size: 80x24, 120x40, 160x40, 200x50, or COLSxROWS.
        #[arg(long, default_value = "80x24")]
        size: String,
        /// Shell binary path. Defaults to $SHELL.
        #[arg(long)]
        shell: Option<String>,
    },

    /// Destroy a terminal by ID.
    Destroy { id: String },

    /// List all active terminals.
    List,

    /// Send a single named key.
    SendKey { id: String, key: String },

    /// Send a sequence of inputs (JSON array, e.g. '[{"text":"vim"},{"key":"Enter"}]').
    SendKeys { id: String, input: String },

    /// Send a mouse event.
    Mouse {
        id: String,
        /// Event: click, press, release, scroll, drag.
        #[arg(long, default_value = "click")]
        event: String,
        /// Button: left, middle, right.
        #[arg(long, default_value = "left")]
        button: String,
        #[arg(long)]
        x: Option<u16>,
        #[arg(long)]
        y: Option<u16>,
        #[arg(long)]
        direction: Option<String>,
        #[arg(long)]
        from_x: Option<u16>,
        #[arg(long)]
        from_y: Option<u16>,
        #[arg(long)]
        to_x: Option<u16>,
        #[arg(long)]
        to_y: Option<u16>,
    },

    /// Read the screen with coordinate overlay.
    Read {
        id: String,
        /// Mode: full, changes, plain.
        #[arg(long, default_value = "full")]
        mode: String,
        #[arg(long)]
        max_lines: Option<u16>,
    },

    /// Read the screen as plain text without coordinates.
    Show { id: String },

    /// Read a range of rows.
    Rows { id: String, from: u16, to: u16 },

    /// Read a rectangular region.
    Region {
        id: String,
        #[arg(long)]
        x: u16,
        #[arg(long)]
        y: u16,
        #[arg(long)]
        w: u16,
        #[arg(long)]
        h: u16,
    },

    /// Get the status of a terminal.
    Status { id: String },

    /// Poll and drain the event queue.
    Events {
        id: String,
        #[arg(long)]
        max: Option<usize>,
    },

    /// Select a rectangular region of text.
    Select {
        id: String,
        #[arg(long)]
        from_row: u16,
        #[arg(long)]
        from_col: u16,
        #[arg(long)]
        to_row: u16,
        #[arg(long)]
        to_col: u16,
    },

    /// Scroll viewport by N rows (positive = down).
    Scroll { id: String, by: i32 },

    /// Resize a terminal.
    Resize { id: String, rows: u16, cols: u16 },

    /// Web viewer controls.
    #[command(subcommand)]
    Viewer(ViewerCommand),

    /// Stop the running daemon.
    Shutdown,
}

#[derive(Subcommand)]
pub enum ViewerCommand {
    /// Start the web viewer.
    Start {
        #[arg(long)]
        port: Option<u16>,
    },
    /// Stop the web viewer.
    Stop,
    /// Show viewer status and address.
    Status,
    /// Print the viewer URL (starting it if needed) — for use with `open`/`xdg-open`.
    Open,
}
