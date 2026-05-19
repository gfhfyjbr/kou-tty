use std::sync::Arc;

use serde_json::{Value, json};

use crate::protocol::{ApiResponse, KeyInput, MouseAction, ReadMode, Request, Size};
use crate::terminal::{Emulator, keys, parse_size};
use crate::viewer::ViewerHandle;

use super::registry::Registry;

pub struct DaemonContext {
    pub registry: Arc<Registry>,
    pub viewer: Arc<ViewerHandle>,
    pub shutdown: tokio::sync::Notify,
}

impl DaemonContext {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            registry: Registry::new(),
            viewer: Arc::new(ViewerHandle::new()),
            shutdown: tokio::sync::Notify::new(),
        })
    }
}

pub async fn handle_request(ctx: Arc<DaemonContext>, request: Request) -> ApiResponse {
    let registry = Arc::clone(&ctx.registry);
    let viewer = Arc::clone(&ctx.viewer);
    let mut result =
        tokio::task::spawn_blocking(move || handle_blocking(registry, viewer, request))
            .await
            .unwrap_or_else(|e| ApiResponse::err("internal", e.to_string()));

    let should_shutdown = result
        .result
        .as_mut()
        .and_then(|v| v.as_object_mut())
        .and_then(|obj| obj.remove("__shutdown__"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if should_shutdown {
        ctx.shutdown.notify_waiters();
    }
    result
}

fn handle_blocking(
    registry: Arc<Registry>,
    viewer: Arc<ViewerHandle>,
    request: Request,
) -> ApiResponse {
    match request {
        Request::Ping => ApiResponse::ok(json!({ "pong": true })),
        Request::TerminalCreate { size, shell } => {
            let (rows, cols) = match size {
                Some(Size::Named(s)) => match parse_size(&s) {
                    Ok(v) => v,
                    Err(e) => return ApiResponse::err("bad_size", e.to_string()),
                },
                Some(Size::Explicit { rows, cols }) => (rows, cols),
                None => (24, 80),
            };
            match registry.create(rows, cols, shell) {
                Ok((id, _emulator)) => {
                    ApiResponse::ok(json!({ "id": id, "rows": rows, "cols": cols }))
                }
                Err(e) => ApiResponse::err("create_failed", e.to_string()),
            }
        }
        Request::TerminalDestroy { id, if_exists } => match registry.destroy(&id) {
            Ok(()) => ApiResponse::ok(json!({ "destroyed": id })),
            Err(e) => {
                if if_exists {
                    ApiResponse::ok(json!({ "destroyed": id, "missing": true }))
                } else {
                    ApiResponse::err_hint(
                        "not_found",
                        e.to_string(),
                        "run `kou-tty terminal list` to see active ids, or pass `--if-exists` to ignore",
                    )
                }
            }
        },
        Request::TerminalList => {
            let items: Vec<Value> = registry
                .list()
                .into_iter()
                .map(|(id, e)| {
                    let size = *e.size.lock().unwrap();
                    let st = e.state.lock().unwrap();
                    json!({
                        "id": id,
                        "rows": size.rows,
                        "cols": size.cols,
                        "process_state": st.process_state,
                        "has_new_content": st.has_new_content(),
                        "exit_status": st.exit_status,
                    })
                })
                .collect();
            ApiResponse::ok(json!({ "terminals": items }))
        }
        Request::TerminalSendKey { id, key } => with_term(&registry, &id, |e| {
            let Some(bytes) = keys::encode_key(&key) else {
                return ApiResponse::err("bad_key", format!("unknown key '{key}'"));
            };
            match e.write_bytes(&bytes) {
                Ok(()) => ApiResponse::ok(json!({ "sent": bytes.len() })),
                Err(err) => ApiResponse::err("write_failed", err.to_string()),
            }
        }),
        Request::TerminalSendKeys { id, input } => with_term(&registry, &id, |e| {
            let mut sent = 0usize;
            for item in &input {
                let bytes = match item {
                    KeyInput::Text { text } => text.as_bytes().to_vec(),
                    KeyInput::Key { key } => match keys::encode_key(key) {
                        Some(b) => b,
                        None => {
                            return ApiResponse::err("bad_key", format!("unknown key '{key}'"));
                        }
                    },
                };
                if let Err(err) = e.write_bytes(&bytes) {
                    return ApiResponse::err("write_failed", err.to_string());
                }
                sent += bytes.len();
            }
            ApiResponse::ok(json!({ "sent": sent }))
        }),
        Request::TerminalMouse { id, action } => with_term(&registry, &id, |e| {
            let bytes_seq = mouse_to_bytes(&action);
            for b in &bytes_seq {
                if let Err(err) = e.write_bytes(b) {
                    return ApiResponse::err("write_failed", err.to_string());
                }
            }
            ApiResponse::ok(json!({ "events": bytes_seq.len() }))
        }),
        Request::TerminalReadScreen {
            id,
            mode,
            max_lines,
        } => with_term(&registry, &id, |e| read_screen(&e, mode, max_lines)),
        Request::TerminalShowScreen { id } => with_term(&registry, &id, |e| {
            let mut st = e.state.lock().unwrap();
            let text = st.grid.plain_text();
            st.mark_read();
            ApiResponse::ok(json!({ "text": text }))
        }),
        Request::TerminalReadRows { id, from, to } => with_term(&registry, &id, |e| {
            let st = e.state.lock().unwrap();
            let from = from.min(st.grid.rows.saturating_sub(1));
            let to = to.min(st.grid.rows.saturating_sub(1));
            let mut out = String::new();
            for r in from..=to {
                out.push_str(&st.grid.row_text(r));
                out.push('\n');
            }
            ApiResponse::ok(json!({ "text": out, "from": from, "to": to }))
        }),
        Request::TerminalReadRegion { id, x, y, w, h } => with_term(&registry, &id, |e| {
            let st = e.state.lock().unwrap();
            let mut lines: Vec<String> = Vec::new();
            for dy in 0..h {
                let r = y.saturating_add(dy);
                if r >= st.grid.rows {
                    break;
                }
                let row_text: String = st
                    .grid
                    .cells
                    .get(r as usize)
                    .map(|line| {
                        line.iter()
                            .skip(x as usize)
                            .take(w as usize)
                            .map(|c| c.ch)
                            .collect()
                    })
                    .unwrap_or_default();
                lines.push(row_text);
            }
            ApiResponse::ok(json!({ "lines": lines }))
        }),
        Request::TerminalStatus { id } => with_term(&registry, &id, |e| {
            let size = *e.size.lock().unwrap();
            let st = e.state.lock().unwrap();
            ApiResponse::ok(json!({
                "id": id,
                "rows": size.rows,
                "cols": size.cols,
                "process_state": st.process_state,
                "has_new_content": st.has_new_content(),
                "exit_status": st.exit_status,
                "cursor": { "row": st.grid.cursor_row, "col": st.grid.cursor_col },
                "bytes_in": st.total_bytes_in,
                "shell": e.shell,
            }))
        }),
        Request::TerminalPollEvents { id, max } => with_term(&registry, &id, |e| {
            let mut st = e.state.lock().unwrap();
            let events = st.drain_events();
            let limit = max.unwrap_or(events.len());
            let taken: Vec<_> = events.into_iter().take(limit).collect();
            ApiResponse::ok(json!({ "events": taken }))
        }),
        Request::TerminalSelect {
            id,
            from_row,
            from_col,
            to_row,
            to_col,
        } => with_term(&registry, &id, |e| {
            let st = e.state.lock().unwrap();
            let mut out = String::new();
            for r in from_row..=to_row.min(st.grid.rows.saturating_sub(1)) {
                let row_chars: Vec<char> = st
                    .grid
                    .cells
                    .get(r as usize)
                    .map(|line| line.iter().map(|c| c.ch).collect())
                    .unwrap_or_default();
                let start = if r == from_row { from_col as usize } else { 0 };
                let end = if r == to_row {
                    (to_col as usize + 1).min(row_chars.len())
                } else {
                    row_chars.len()
                };
                if start < end {
                    out.extend(&row_chars[start..end]);
                }
                if r != to_row {
                    out.push('\n');
                }
            }
            ApiResponse::ok(json!({ "text": out }))
        }),
        Request::TerminalScroll { id, by } => with_term(&registry, &id, |_e| {
            ApiResponse::ok(
                json!({ "scrolled": by, "note": "scrollback rendering is consumer-side" }),
            )
        }),
        Request::TerminalResize { id, rows, cols } => {
            with_term(&registry, &id, |e| match e.resize(rows, cols) {
                Ok(()) => ApiResponse::ok(json!({ "rows": rows, "cols": cols })),
                Err(err) => ApiResponse::err("resize_failed", err.to_string()),
            })
        }
        Request::ViewerStart { port } => match viewer.start(Arc::clone(&registry), port) {
            Ok(addr) => ApiResponse::ok(json!({ "address": addr })),
            Err(e) => ApiResponse::err_hint(
                "viewer_failed",
                e.to_string(),
                "pass --port N to try a different port, or run `kou-tty viewer stop` first",
            ),
        },
        Request::ViewerStop => match viewer.stop() {
            Ok(()) => ApiResponse::ok(json!({ "stopped": true })),
            Err(e) => ApiResponse::err("viewer_failed", e.to_string()),
        },
        Request::ViewerStatus => {
            let addr = viewer.address();
            ApiResponse::ok(json!({ "running": addr.is_some(), "address": addr }))
        }
        Request::Shutdown => ApiResponse::ok(json!({ "shutdown": true, "__shutdown__": true })),
    }
}

fn with_term<F>(registry: &Registry, id: &str, f: F) -> ApiResponse
where
    F: FnOnce(Arc<Emulator>) -> ApiResponse,
{
    match registry.get(id) {
        Ok(e) => f(e),
        Err(err) => ApiResponse::err_hint(
            "not_found",
            err.to_string(),
            "run `kou-tty terminal list` to see active ids",
        ),
    }
}

fn read_screen(e: &Arc<Emulator>, mode: ReadMode, max_lines: Option<u16>) -> ApiResponse {
    let mut st = e.state.lock().unwrap();
    let size = *e.size.lock().unwrap();
    let cap = max_lines.unwrap_or(200).min(200);
    match mode {
        ReadMode::Plain => {
            let text = st.grid.plain_text();
            st.mark_read();
            ApiResponse::ok(json!({ "text": text, "rows": size.rows, "cols": size.cols }))
        }
        ReadMode::Full => {
            let text = render_with_coords(&st.grid, None);
            st.mark_read();
            ApiResponse::ok(json!({
                "text": text,
                "rows": size.rows,
                "cols": size.cols,
                "cursor": { "row": st.grid.cursor_row, "col": st.grid.cursor_col },
            }))
        }
        ReadMode::Changes => {
            let dirty = st.grid.take_dirty();
            let limited: Vec<u16> = dirty.into_iter().take(cap as usize).collect();
            let text = render_with_coords(&st.grid, Some(&limited));
            st.mark_read();
            ApiResponse::ok(json!({
                "text": text,
                "rows": limited,
                "cursor": { "row": st.grid.cursor_row, "col": st.grid.cursor_col },
            }))
        }
    }
}

fn render_with_coords(grid: &crate::terminal::Grid, only_rows: Option<&[u16]>) -> String {
    let mut out = String::new();
    // header
    out.push_str("     ");
    for c in 0..grid.cols {
        out.push(((c % 10) as u8 + b'0') as char);
    }
    out.push('\n');
    let rows: Box<dyn Iterator<Item = u16>> = match only_rows {
        Some(r) => Box::new(r.iter().copied()),
        None => Box::new(0..grid.rows),
    };
    for r in rows {
        if r >= grid.rows {
            continue;
        }
        out.push_str(&format!("{r:>3}: "));
        out.push_str(&grid.row_text(r));
        out.push('\n');
    }
    out
}

fn mouse_to_bytes(action: &MouseAction) -> Vec<Vec<u8>> {
    match action {
        MouseAction::Click { button, x, y } => {
            let b = button_code(button);
            vec![
                keys::encode_mouse_sgr(b, *x, *y, true),
                keys::encode_mouse_sgr(b, *x, *y, false),
            ]
        }
        MouseAction::Press { button, x, y } => {
            vec![keys::encode_mouse_sgr(button_code(button), *x, *y, true)]
        }
        MouseAction::Release { button, x, y } => {
            vec![keys::encode_mouse_sgr(button_code(button), *x, *y, false)]
        }
        MouseAction::Scroll { direction, x, y } => {
            let b = if direction.eq_ignore_ascii_case("up") {
                64
            } else {
                65
            };
            vec![keys::encode_mouse_sgr(b, *x, *y, true)]
        }
        MouseAction::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
        } => vec![
            keys::encode_mouse_sgr(0, *from_x, *from_y, true),
            keys::encode_mouse_sgr(32, *to_x, *to_y, true),
            keys::encode_mouse_sgr(0, *to_x, *to_y, false),
        ],
    }
}

fn button_code(name: &str) -> u16 {
    match name.to_lowercase().as_str() {
        "right" => 2,
        "middle" => 1,
        _ => 0,
    }
}
