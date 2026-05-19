use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::daemon::{DaemonClient, default_socket_path, run as run_daemon};
use crate::protocol::{ApiResponse, KeyInput, MouseAction, ReadMode, Request, Size, exit_code_for};

use super::{Cli, Command, TerminalCommand, ViewerCommand};

pub async fn dispatch(cli: Cli) -> Result<()> {
    let socket = cli.socket.clone().unwrap_or_else(default_socket_path);
    let quiet = cli.quiet;
    let compact = cli.compact;
    match cli.command {
        Command::Daemon => run_daemon(&socket).await,
        Command::Json => run_json_bridge(&socket).await,
        Command::Repl => run_repl(&socket).await,
        cmd => run_client_command(&socket, cmd, quiet, compact).await,
    }
}

async fn run_client_command(
    socket: &PathBuf,
    cmd: Command,
    quiet: bool,
    compact: bool,
) -> Result<()> {
    let client = DaemonClient::new(socket.clone());
    let request = build_request(&cmd)?;
    let response = client.send(&request).await?;

    write_stdout(&response, &cmd, quiet, compact)?;
    write_stderr_on_error(&response);

    let code = exit_code_for(&response);
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

fn write_stdout(response: &ApiResponse, cmd: &Command, quiet: bool, compact: bool) -> Result<()> {
    if quiet {
        let bare = quiet_value(response, cmd);
        if !bare.is_empty() {
            println!("{bare}");
        }
        return Ok(());
    }
    if compact {
        println!("{}", serde_json::to_string(response)?);
    } else {
        println!("{}", serde_json::to_string_pretty(response)?);
    }
    Ok(())
}

fn write_stderr_on_error(response: &ApiResponse) {
    if response.ok {
        return;
    }
    let Some(err) = response.error.as_ref() else {
        return;
    };
    eprintln!("error[{}]: {}", err.code, err.message);
    if let Some(hint) = &err.suggestion {
        eprintln!("hint: {hint}");
    }
}

fn quiet_value(response: &ApiResponse, cmd: &Command) -> String {
    if !response.ok {
        return String::new();
    }
    let Some(result) = response.result.as_ref() else {
        return String::new();
    };
    match cmd {
        Command::Terminal(t) => terminal_quiet(t, result),
        Command::Viewer(v) => viewer_quiet(v, result),
        Command::Shutdown => String::new(),
        Command::Daemon | Command::Json | Command::Repl => String::new(),
    }
}

fn terminal_quiet(cmd: &TerminalCommand, result: &Value) -> String {
    match cmd {
        TerminalCommand::Create { .. } => take_str(result, "id"),
        TerminalCommand::List => result
            .get("terminals")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.get("id").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        TerminalCommand::Show { .. }
        | TerminalCommand::Rows { .. }
        | TerminalCommand::Select { .. } => take_str(result, "text"),
        TerminalCommand::Read { .. } => take_str(result, "text"),
        TerminalCommand::Region { .. } => result
            .get("lines")
            .and_then(|l| l.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default(),
        TerminalCommand::Status { .. } => take_str(result, "process_state"),
        TerminalCommand::Events { .. } => result
            .get("events")
            .map(|e| e.to_string())
            .unwrap_or_default(),
        TerminalCommand::Destroy { .. }
        | TerminalCommand::SendKey { .. }
        | TerminalCommand::SendKeys { .. }
        | TerminalCommand::Mouse { .. }
        | TerminalCommand::Scroll { .. }
        | TerminalCommand::Resize { .. } => String::new(),
    }
}

fn viewer_quiet(cmd: &ViewerCommand, result: &Value) -> String {
    match cmd {
        ViewerCommand::Start { .. } | ViewerCommand::Open => take_str(result, "address"),
        ViewerCommand::Status => take_str(result, "address"),
        ViewerCommand::Stop => String::new(),
    }
}

fn take_str(result: &Value, key: &str) -> String {
    result
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn build_request(cmd: &Command) -> Result<Request> {
    match cmd {
        Command::Daemon | Command::Json | Command::Repl => {
            anyhow::bail!("internal: build_request called for special command")
        }
        Command::Shutdown => Ok(Request::Shutdown),
        Command::Terminal(t) => build_terminal_request(t),
        Command::Viewer(v) => Ok(build_viewer_request(v)),
    }
}

fn build_terminal_request(cmd: &TerminalCommand) -> Result<Request> {
    match cmd {
        TerminalCommand::Create { size, shell } => Ok(Request::TerminalCreate {
            size: Some(Size::Named(size.clone())),
            shell: shell.clone(),
        }),
        TerminalCommand::Destroy { id, if_exists } => Ok(Request::TerminalDestroy {
            id: id.clone(),
            if_exists: *if_exists,
        }),
        TerminalCommand::List => Ok(Request::TerminalList),
        TerminalCommand::SendKey { id, key } => Ok(Request::TerminalSendKey {
            id: id.clone(),
            key: key.clone(),
        }),
        TerminalCommand::SendKeys { id, input } => {
            let parsed: Vec<KeyInput> = serde_json::from_str(input).context(
                "`input` must be a JSON array like [{\"text\":\"hi\"},{\"key\":\"Enter\"}]",
            )?;
            Ok(Request::TerminalSendKeys {
                id: id.clone(),
                input: parsed,
            })
        }
        TerminalCommand::Mouse {
            id,
            event,
            button,
            x,
            y,
            direction,
            from_x,
            from_y,
            to_x,
            to_y,
        } => {
            let action = match event.to_lowercase().as_str() {
                "click" => MouseAction::Click {
                    button: button.clone(),
                    x: x.context("--x required")?,
                    y: y.context("--y required")?,
                },
                "press" => MouseAction::Press {
                    button: button.clone(),
                    x: x.context("--x required")?,
                    y: y.context("--y required")?,
                },
                "release" => MouseAction::Release {
                    button: button.clone(),
                    x: x.context("--x required")?,
                    y: y.context("--y required")?,
                },
                "scroll" => MouseAction::Scroll {
                    direction: direction
                        .clone()
                        .context("--direction required for scroll")?,
                    x: x.context("--x required")?,
                    y: y.context("--y required")?,
                },
                "drag" => MouseAction::Drag {
                    from_x: from_x.context("--from-x required")?,
                    from_y: from_y.context("--from-y required")?,
                    to_x: to_x.context("--to-x required")?,
                    to_y: to_y.context("--to-y required")?,
                },
                other => anyhow::bail!("unknown mouse event '{other}'"),
            };
            Ok(Request::TerminalMouse {
                id: id.clone(),
                action,
            })
        }
        TerminalCommand::Read {
            id,
            mode,
            max_lines,
        } => Ok(Request::TerminalReadScreen {
            id: id.clone(),
            mode: parse_read_mode(mode)?,
            max_lines: *max_lines,
        }),
        TerminalCommand::Show { id } => Ok(Request::TerminalShowScreen { id: id.clone() }),
        TerminalCommand::Rows { id, from, to } => Ok(Request::TerminalReadRows {
            id: id.clone(),
            from: *from,
            to: *to,
        }),
        TerminalCommand::Region { id, x, y, w, h } => Ok(Request::TerminalReadRegion {
            id: id.clone(),
            x: *x,
            y: *y,
            w: *w,
            h: *h,
        }),
        TerminalCommand::Status { id } => Ok(Request::TerminalStatus { id: id.clone() }),
        TerminalCommand::Events { id, max } => Ok(Request::TerminalPollEvents {
            id: id.clone(),
            max: *max,
        }),
        TerminalCommand::Select {
            id,
            from_row,
            from_col,
            to_row,
            to_col,
        } => Ok(Request::TerminalSelect {
            id: id.clone(),
            from_row: *from_row,
            from_col: *from_col,
            to_row: *to_row,
            to_col: *to_col,
        }),
        TerminalCommand::Scroll { id, by } => Ok(Request::TerminalScroll {
            id: id.clone(),
            by: *by,
        }),
        TerminalCommand::Resize { id, rows, cols } => Ok(Request::TerminalResize {
            id: id.clone(),
            rows: *rows,
            cols: *cols,
        }),
    }
}

fn build_viewer_request(cmd: &ViewerCommand) -> Request {
    match cmd {
        ViewerCommand::Start { port } => Request::ViewerStart { port: *port },
        ViewerCommand::Stop => Request::ViewerStop,
        ViewerCommand::Status => Request::ViewerStatus,
        ViewerCommand::Open => Request::ViewerStart { port: None },
    }
}

fn parse_read_mode(s: &str) -> Result<ReadMode> {
    match s.to_lowercase().as_str() {
        "full" => Ok(ReadMode::Full),
        "changes" => Ok(ReadMode::Changes),
        "plain" => Ok(ReadMode::Plain),
        other => anyhow::bail!("unknown read mode '{other}'; expected full|changes|plain"),
    }
}

async fn run_json_bridge(socket: &PathBuf) -> Result<()> {
    let client = DaemonClient::new(socket.clone());
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response: Value = match serde_json::from_str::<Request>(trimmed) {
            Ok(request) => match client.send(&request).await {
                Ok(resp) => serde_json::to_value(resp)?,
                Err(e) => {
                    json!({ "ok": false, "error": { "code": "client_error", "message": e.to_string() } })
                }
            },
            Err(e) => {
                json!({ "ok": false, "error": { "code": "bad_request", "message": e.to_string() } })
            }
        };
        let mut bytes = serde_json::to_vec(&response)?;
        bytes.push(b'\n');
        stdout.write_all(&bytes).await?;
        stdout.flush().await?;
    }
    Ok(())
}

async fn run_repl(socket: &PathBuf) -> Result<()> {
    let client = DaemonClient::new(socket.clone());
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    stdout
        .write_all(b"kou-tty repl - type JSON-RPC lines, or 'help'/'quit'\n")
        .await?;
    stdout.flush().await?;
    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        match trimmed {
            "" => continue,
            "quit" | "exit" => break,
            "help" => {
                stdout
                    .write_all(b"Examples:\n  {\"method\":\"ping\"}\n  {\"method\":\"terminal_create\",\"params\":{}}\n")
                    .await?;
                continue;
            }
            _ => {}
        }
        match serde_json::from_str::<Request>(trimmed) {
            Ok(req) => match client.send(&req).await {
                Ok(resp) => {
                    stdout
                        .write_all(serde_json::to_string_pretty(&resp)?.as_bytes())
                        .await?;
                    stdout.write_all(b"\n").await?;
                }
                Err(e) => {
                    stdout.write_all(format!("error: {e}\n").as_bytes()).await?;
                }
            },
            Err(e) => {
                stdout
                    .write_all(format!("parse error: {e}\n").as_bytes())
                    .await?;
            }
        }
        stdout.flush().await?;
    }
    Ok(())
}
