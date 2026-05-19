pub mod commands;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
#[clap(rename_all = "lower")]
pub enum ColorWhen {
    Always,
    #[default]
    Auto,
    Never,
}

const LONG_ABOUT: &str = "\
kou-tty is a headless terminal emulator. It spawns a PTY for each terminal,
drains output through a VT/ANSI parser into an in-memory grid, and exposes
a noun-verb CLI plus a JSON-RPC stdin/stdout bridge for AI agents.

Output:
  default      bare value (id, plain text, process_state, ...) — best for $(...)
  --json / -j  pretty JSON envelope { ok, result | error }
  --compact    single-line JSON (implies --json)

Errors always print `error[<code>]: <message>` and `hint: ...` to stderr; the
stdout side stays empty or a clean JSON object so it remains pipe-safe.

Examples:
  ID=$(kou-tty terminal create)
  kou-tty terminal send-keys \"$ID\" '[{\"text\":\"echo hi\"},{\"key\":\"Enter\"}]'
  kou-tty terminal show \"$ID\"
  kou-tty terminal destroy \"$ID\" --if-exists

  # full JSON when scripting with jq
  kou-tty --json terminal status \"$ID\" | jq -r .result.process_state

  # JSON-RPC bridge (one request per line, one response per line)
  printf '{\"method\":\"ping\"}\\n' | kou-tty json

Exit codes:
  0  success
  1  general failure
  2  usage error or bad request
  3  terminal not found
  5  conflict / already exists
";

#[derive(Parser)]
#[command(
    name = "kou-tty",
    version,
    about = "Headless in-memory terminal emulator CLI",
    long_about = LONG_ABOUT,
    disable_help_subcommand = true,
)]
pub struct Cli {
    /// Path to daemon socket. Default: $XDG_RUNTIME_DIR/kou-tty-<uid>.sock or $TMPDIR/...
    #[arg(long, global = true)]
    pub socket: Option<PathBuf>,

    /// Print the full JSON envelope instead of the bare value.
    #[arg(long, short = 'j', global = true)]
    pub json: bool,

    /// Print single-line JSON (implies --json).
    #[arg(long, short = 'c', global = true)]
    pub compact: bool,

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

    /// Stop the running daemon. Idempotent (no error if daemon not running).
    Shutdown,

    /// Operate on terminals.
    #[command(subcommand)]
    Terminal(TerminalCommand),

    /// Web viewer controls.
    #[command(subcommand)]
    Viewer(ViewerCommand),
}

#[derive(Subcommand)]
pub enum TerminalCommand {
    /// Create a new terminal. Prints the bare id by default.
    #[command(long_about = "Spawn a fresh PTY with the given size and shell.\n\n\
Examples:\n  \
ID=$(kou-tty terminal create)\n  \
kou-tty terminal create --size 120x40 --shell /bin/zsh\n  \
kou-tty --json terminal create   # full JSON envelope")]
    Create {
        /// Size: 80x24, 120x40, 160x40, 200x50, or COLSxROWS.
        #[arg(long, default_value = "80x24")]
        size: String,
        /// Shell binary path. Defaults to $SHELL or /bin/bash.
        #[arg(long)]
        shell: Option<String>,
        /// Pixel width of a single cell (default 8). TUI apps read this via
        /// TIOCGWINSZ to compute aspect ratio.
        #[arg(long)]
        cell_width: Option<u16>,
        /// Pixel height of a single cell (default 16).
        #[arg(long)]
        cell_height: Option<u16>,
    },

    /// Destroy a terminal by id.
    #[command(long_about = "Kill the child process and free the PTY.\n\n\
Examples:\n  \
kou-tty terminal destroy a0\n  \
kou-tty terminal destroy a0 --if-exists   # idempotent: ok even if already gone")]
    Destroy {
        id: String,
        /// Treat a missing terminal as success (idempotent).
        #[arg(long)]
        if_exists: bool,
    },

    /// List all active terminals.
    List,

    /// Send a single named key (Enter, Tab, Escape, Ctrl+c, ...).
    SendKey {
        id: String,
        /// Key name. Examples: Enter, Tab, Escape, Backspace, Up, F5, Ctrl+c, Alt+f
        key: String,
    },

    /// Send a sequence of inputs as a JSON array.
    #[command(
        long_about = "Send a JSON array of {text,key} items to the terminal.\n\n\
Examples:\n  \
kou-tty terminal send-keys a0 '[{\"text\":\"vim file.txt\"},{\"key\":\"Enter\"}]'\n  \
kou-tty terminal send-keys a0 '[{\"key\":\"Escape\"},{\"text\":\":q!\"},{\"key\":\"Enter\"}]'"
    )]
    SendKeys {
        id: String,
        /// JSON array of inputs.
        input: String,
    },

    /// Send a mouse event (SGR-1006 encoding).
    Mouse {
        id: String,
        /// Event: click, press, release, scroll, drag.
        #[arg(long, default_value = "click")]
        event: String,
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

    /// Read the screen with a coordinate overlay.
    #[command(long_about = "Read modes:\n  \
full     every row with a column ruler\n  \
changes  only rows that changed since the last read (token-efficient)\n  \
plain    every row, no overlay\n\n\
Use --color always|auto|never to re-emit the SGR escape sequences captured\n\
from the PTY. Default is auto (color when stdout is a TTY).")]
    Read {
        id: String,
        #[arg(long, default_value = "full")]
        mode: String,
        #[arg(long)]
        max_lines: Option<u16>,
        /// Re-emit ANSI colour codes captured from the PTY.
        #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
        color: ColorWhen,
    },

    /// Read the screen as plain text without coordinates.
    Show {
        id: String,
        /// Re-emit ANSI colour codes captured from the PTY.
        #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
        color: ColorWhen,
    },

    /// Read a range of rows.
    Rows {
        id: String,
        from: u16,
        to: u16,
        #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
        color: ColorWhen,
    },

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
        #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
        color: ColorWhen,
    },

    /// Get terminal status (process state, has_new_content, cursor, ...).
    Status { id: String },

    /// Poll and drain the event queue.
    Events {
        id: String,
        #[arg(long)]
        max: Option<usize>,
    },

    /// Select a rectangular region of text. Pure read (does not modify the screen).
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
        #[arg(long, value_enum, default_value_t = ColorWhen::Auto)]
        color: ColorWhen,
    },

    /// Scroll viewport by N rows (positive = down).
    Scroll { id: String, by: i32 },

    /// Resize a terminal.
    Resize { id: String, rows: u16, cols: u16 },
}

#[derive(Subcommand)]
pub enum ViewerCommand {
    /// Start the web viewer (binds 127.0.0.1, auto-probes the next 10 ports).
    Start {
        #[arg(long)]
        port: Option<u16>,
    },
    /// Stop the web viewer.
    Stop,
    /// Show viewer status and address.
    Status,
    /// Start the viewer if needed and print its URL.
    Open,
}
