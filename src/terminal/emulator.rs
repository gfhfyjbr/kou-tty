use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use super::ansi::AnsiHandler;
use super::events::{TerminalEvent, now_ms};
use super::grid::Grid;
use super::state::ProcessState;

const EVENT_RING_CAPACITY: usize = 1024;
const IDLE_THRESHOLD_MS: u128 = 500;

/// Reasonable defaults for a monospace character cell (≈8x16 pixels). TUI
/// frameworks read `pixel_width`/`pixel_height` via TIOCGWINSZ to compute
/// font aspect ratio; leaving them at 0 makes some apps assume square cells
/// and render at half resolution.
pub const DEFAULT_CELL_WIDTH: u16 = 8;
pub const DEFAULT_CELL_HEIGHT: u16 = 16;

pub struct EmulatorState {
    pub grid: Grid,
    pub events: VecDeque<TerminalEvent>,
    pub process_state: ProcessState,
    pub last_output_at: Instant,
    pub last_read_seq: u64,
    pub exit_status: Option<i32>,
    pub total_bytes_in: u64,
}

impl EmulatorState {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            grid: Grid::new(rows, cols),
            events: VecDeque::with_capacity(EVENT_RING_CAPACITY),
            process_state: ProcessState::Running,
            last_output_at: Instant::now(),
            last_read_seq: 0,
            exit_status: None,
            total_bytes_in: 0,
        }
    }

    pub fn push_event(&mut self, event: TerminalEvent) {
        if self.events.len() == EVENT_RING_CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }

    pub fn drain_events(&mut self) -> Vec<TerminalEvent> {
        self.events.drain(..).collect()
    }

    pub fn has_new_content(&self) -> bool {
        self.grid.seq != self.last_read_seq
    }

    pub fn mark_read(&mut self) {
        self.last_read_seq = self.grid.seq;
    }
}

pub struct Emulator {
    pub size: Mutex<PtySize>,
    pub cell: Mutex<CellPixels>,
    pub state: Arc<Mutex<EmulatorState>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    writer: Mutex<Box<dyn Write + Send>>,
    child: Mutex<Box<dyn Child + Send + Sync>>,
    pub shell: String,
}

#[derive(Clone, Copy, Debug)]
pub struct CellPixels {
    pub width: u16,
    pub height: u16,
}

impl Default for CellPixels {
    fn default() -> Self {
        Self {
            width: DEFAULT_CELL_WIDTH,
            height: DEFAULT_CELL_HEIGHT,
        }
    }
}

impl Emulator {
    pub fn spawn(
        rows: u16,
        cols: u16,
        shell: Option<String>,
        cell: CellPixels,
    ) -> Result<Arc<Self>> {
        let size = PtySize {
            rows,
            cols,
            pixel_width: cols.saturating_mul(cell.width),
            pixel_height: rows.saturating_mul(cell.height),
        };
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(size).context("failed to open pty")?;

        let shell_path = shell.unwrap_or_else(default_shell);
        let mut cmd = CommandBuilder::new(&shell_path);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        if let Ok(home) = std::env::var("HOME") {
            cmd.cwd(home);
        }
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("failed to spawn shell")?;
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone pty reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("failed to take pty writer")?;

        let state = Arc::new(Mutex::new(EmulatorState::new(rows, cols)));

        let emulator = Arc::new(Self {
            size: Mutex::new(size),
            cell: Mutex::new(cell),
            state: Arc::clone(&state),
            master: Mutex::new(pair.master),
            writer: Mutex::new(writer),
            child: Mutex::new(child),
            shell: shell_path,
        });

        spawn_reader_thread(Arc::clone(&state), reader);
        spawn_monitor_thread(Arc::clone(&emulator));

        Ok(emulator)
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        let cell = *self.cell.lock().unwrap();
        let new_size = PtySize {
            rows,
            cols,
            pixel_width: cols.saturating_mul(cell.width),
            pixel_height: rows.saturating_mul(cell.height),
        };
        self.master
            .lock()
            .unwrap()
            .resize(new_size)
            .context("pty resize failed")?;
        *self.size.lock().unwrap() = new_size;
        self.state.lock().unwrap().grid.resize(rows, cols);
        Ok(())
    }

    pub fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock().unwrap();
        writer.write_all(bytes).context("pty write failed")?;
        writer.flush().ok();
        Ok(())
    }

    pub fn try_wait(&self) -> Option<i32> {
        let mut child = self.child.lock().unwrap();
        match child.try_wait() {
            Ok(Some(status)) => Some(status.exit_code() as i32),
            _ => None,
        }
    }

    pub fn kill(&self) -> Result<()> {
        let mut child = self.child.lock().unwrap();
        child.kill().context("failed to kill child")?;
        Ok(())
    }
}

fn spawn_reader_thread(state: Arc<Mutex<EmulatorState>>, mut reader: Box<dyn Read + Send>) {
    thread::spawn(move || {
        let mut parser = vte::Parser::new();
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let mut st = state.lock().unwrap();
                    st.last_output_at = Instant::now();
                    st.total_bytes_in = st.total_bytes_in.saturating_add(n as u64);
                    let before_seq = st.grid.seq;
                    let st = &mut *st;
                    let mut local_events: Vec<TerminalEvent> = Vec::new();
                    {
                        let mut handler = AnsiHandler::new(&mut st.grid, &mut local_events);
                        parser.advance(&mut handler, &buf[..n]);
                    }
                    for ev in local_events.drain(..) {
                        st.push_event(ev);
                    }
                    if st.grid.seq != before_seq {
                        let rows: Vec<u16> = st.grid.dirty_rows.iter().copied().collect();
                        if !rows.is_empty() {
                            st.push_event(TerminalEvent::ScreenChanged {
                                rows,
                                timestamp_ms: now_ms(),
                            });
                        }
                        if st.process_state != ProcessState::Running {
                            let prev = st.process_state;
                            st.process_state = ProcessState::Running;
                            st.push_event(TerminalEvent::ProcessStateChanged {
                                from: prev,
                                to: ProcessState::Running,
                                timestamp_ms: now_ms(),
                            });
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_monitor_thread(emulator: Arc<Emulator>) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(100));
            if let Some(code) = emulator.try_wait() {
                let mut st = emulator.state.lock().unwrap();
                if st.process_state != ProcessState::Exited {
                    let prev = st.process_state;
                    st.process_state = ProcessState::Exited;
                    st.exit_status = Some(code);
                    st.push_event(TerminalEvent::CommandFinished {
                        exit_code: Some(code),
                        timestamp_ms: now_ms(),
                    });
                    st.push_event(TerminalEvent::ProcessStateChanged {
                        from: prev,
                        to: ProcessState::Exited,
                        timestamp_ms: now_ms(),
                    });
                }
                return;
            }
            let mut st = emulator.state.lock().unwrap();
            if st.process_state == ProcessState::Exited {
                return;
            }
            let elapsed = st.last_output_at.elapsed().as_millis();
            if elapsed > IDLE_THRESHOLD_MS && st.process_state == ProcessState::Running {
                let new_state = if looks_like_prompt(&st.grid) {
                    ProcessState::WaitingForInput
                } else {
                    ProcessState::Idle
                };
                let prev = st.process_state;
                st.process_state = new_state;
                st.push_event(TerminalEvent::ProcessStateChanged {
                    from: prev,
                    to: new_state,
                    timestamp_ms: now_ms(),
                });
                if new_state == ProcessState::WaitingForInput {
                    st.push_event(TerminalEvent::WaitingForInput {
                        timestamp_ms: now_ms(),
                    });
                }
            }
        }
    });
}

fn looks_like_prompt(grid: &Grid) -> bool {
    let row = grid.cursor_row;
    let text = grid.row_text(row);
    let trimmed = text.trim_end();
    let last = trimmed.chars().rev().take(3).collect::<String>();
    last.chars().any(|c| matches!(c, '$' | '#' | '%' | '>'))
}

fn default_shell() -> String {
    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.is_empty() {
            return shell;
        }
    }
    if cfg!(target_os = "windows") {
        "cmd.exe".to_owned()
    } else {
        "/bin/bash".to_owned()
    }
}
