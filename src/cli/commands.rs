use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::daemon::{DaemonClient, default_socket_path, run as run_daemon};
use crate::protocol::{KeyInput, MouseAction, ReadMode, Request, Size};

use super::{Cli, Command, ViewerCommand};

pub async fn dispatch(cli: Cli) -> Result<()> {
    let socket = cli.socket.clone().unwrap_or_else(default_socket_path);
    match cli.command {
        Command::Daemon => run_daemon(&socket).await,
        Command::Json => run_json_bridge(&socket).await,
        Command::Repl => run_repl(&socket).await,
        cmd => run_client_command(&socket, cmd).await,
    }
}

async fn run_client_command(socket: &PathBuf, cmd: Command) -> Result<()> {
    let client = DaemonClient::new(socket.clone());
    let request = build_request(cmd)?;
    let response = client.send(&request).await?;
    let pretty = serde_json::to_string_pretty(&response)?;
    println!("{pretty}");
    if !response.ok {
        std::process::exit(1);
    }
    Ok(())
}

fn build_request(cmd: Command) -> Result<Request> {
    match cmd {
        Command::Daemon | Command::Json | Command::Repl => {
            anyhow::bail!("internal: build_request called for special command")
        }
        Command::Create { size, shell } => Ok(Request::TerminalCreate {
            size: Some(Size::Named(size)),
            shell,
        }),
        Command::Destroy { id } => Ok(Request::TerminalDestroy { id }),
        Command::List => Ok(Request::TerminalList),
        Command::SendKey { id, key } => Ok(Request::TerminalSendKey { id, key }),
        Command::SendKeys { id, input } => {
            let parsed: Vec<KeyInput> = serde_json::from_str(&input).context(
                "`input` must be a JSON array like [{\"text\":\"hi\"},{\"key\":\"Enter\"}]",
            )?;
            Ok(Request::TerminalSendKeys { id, input: parsed })
        }
        Command::Mouse {
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
                    button,
                    x: x.context("--x required")?,
                    y: y.context("--y required")?,
                },
                "press" => MouseAction::Press {
                    button,
                    x: x.context("--x required")?,
                    y: y.context("--y required")?,
                },
                "release" => MouseAction::Release {
                    button,
                    x: x.context("--x required")?,
                    y: y.context("--y required")?,
                },
                "scroll" => MouseAction::Scroll {
                    direction: direction.context("--direction required for scroll")?,
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
            Ok(Request::TerminalMouse { id, action })
        }
        Command::Read {
            id,
            mode,
            max_lines,
        } => {
            let mode = parse_read_mode(&mode)?;
            Ok(Request::TerminalReadScreen {
                id,
                mode,
                max_lines,
            })
        }
        Command::Show { id } => Ok(Request::TerminalShowScreen { id }),
        Command::Rows { id, from, to } => Ok(Request::TerminalReadRows { id, from, to }),
        Command::Region { id, x, y, w, h } => Ok(Request::TerminalReadRegion { id, x, y, w, h }),
        Command::Status { id } => Ok(Request::TerminalStatus { id }),
        Command::Events { id, max } => Ok(Request::TerminalPollEvents { id, max }),
        Command::Select {
            id,
            from_row,
            from_col,
            to_row,
            to_col,
        } => Ok(Request::TerminalSelect {
            id,
            from_row,
            from_col,
            to_row,
            to_col,
        }),
        Command::Scroll { id, by } => Ok(Request::TerminalScroll { id, by }),
        Command::Resize { id, rows, cols } => Ok(Request::TerminalResize { id, rows, cols }),
        Command::Viewer(viewer) => match viewer {
            ViewerCommand::Start { port } => Ok(Request::ViewerStart { port }),
            ViewerCommand::Stop => Ok(Request::ViewerStop),
            ViewerCommand::Status => Ok(Request::ViewerStatus),
            ViewerCommand::Open => Ok(Request::ViewerStart { port: None }),
        },
        Command::Shutdown => Ok(Request::Shutdown),
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
