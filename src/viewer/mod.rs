use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use std::sync::Mutex;

use anyhow::{Context, Result};
use axum::Router;
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use serde_json::json;
use tokio::net::TcpListener;

use crate::daemon::registry::Registry;

const DEFAULT_PORT: u16 = 8039;
const PORT_PROBE_LIMIT: u16 = 10;
const INDEX_HTML: &str = include_str!("index.html");

pub struct ViewerHandle {
    runtime: Option<tokio::runtime::Handle>,
    state: Mutex<ViewerState>,
}

#[derive(Default)]
struct ViewerState {
    address: Option<String>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl ViewerHandle {
    pub fn new() -> Self {
        Self {
            runtime: tokio::runtime::Handle::try_current().ok(),
            state: Mutex::new(ViewerState::default()),
        }
    }

    pub fn address(&self) -> Option<String> {
        self.state.lock().unwrap().address.clone()
    }

    pub fn start(&self, registry: Arc<Registry>, port: Option<u16>) -> Result<String> {
        let Some(runtime) = self.runtime.clone() else {
            anyhow::bail!("viewer requires tokio runtime");
        };
        let mut state = self.state.lock().unwrap();
        if let Some(addr) = &state.address {
            return Ok(addr.clone());
        }

        let (tx, rx) = tokio::sync::oneshot::channel();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<SocketAddr>>(1);
        let registry_clone = Arc::clone(&registry);
        let runtime_clone = runtime.clone();
        runtime.spawn(async move {
            match listen(port.unwrap_or(DEFAULT_PORT)).await {
                Ok((listener, addr)) => {
                    ready_tx.send(Ok(addr)).ok();
                    serve(listener, registry_clone, rx, runtime_clone).await;
                }
                Err(e) => {
                    ready_tx.send(Err(e)).ok();
                }
            }
        });

        let addr = ready_rx
            .recv_timeout(Duration::from_secs(2))
            .context("viewer ready timeout")??;
        let formatted = format!("http://{}", addr);
        state.address = Some(formatted.clone());
        state.shutdown = Some(tx);
        Ok(formatted)
    }

    pub fn stop(&self) -> Result<()> {
        let mut state = self.state.lock().unwrap();
        if let Some(tx) = state.shutdown.take() {
            tx.send(()).ok();
        }
        state.address = None;
        Ok(())
    }
}

async fn listen(start_port: u16) -> Result<(TcpListener, SocketAddr)> {
    for offset in 0..=PORT_PROBE_LIMIT {
        let port = start_port.saturating_add(offset);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                let local = listener.local_addr()?;
                return Ok((listener, local));
            }
            Err(_) => continue,
        }
    }
    anyhow::bail!(
        "no free port in range {start_port}..={}",
        start_port + PORT_PROBE_LIMIT
    )
}

async fn serve(
    listener: TcpListener,
    registry: Arc<Registry>,
    shutdown: tokio::sync::oneshot::Receiver<()>,
    _runtime: tokio::runtime::Handle,
) {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/terminals", get(list_terminals))
        .route("/api/terminals/:id", get(show_terminal))
        .route("/ws/terminals/:id", get(ws_terminal))
        .with_state(registry);

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
        })
        .await
    {
        tracing::warn!(error = %e, "viewer server stopped");
    }
}

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn list_terminals(State(registry): State<Arc<Registry>>) -> impl IntoResponse {
    let items: Vec<_> = registry
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
            })
        })
        .collect();
    axum::Json(json!({ "terminals": items }))
}

async fn show_terminal(
    State(registry): State<Arc<Registry>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match registry.get(&id) {
        Ok(e) => {
            let size = *e.size.lock().unwrap();
            let st = e.state.lock().unwrap();
            (
                StatusCode::OK,
                axum::Json(json!({
                    "id": id,
                    "rows": size.rows,
                    "cols": size.cols,
                    "process_state": st.process_state,
                    "text": st.grid.plain_text(),
                })),
            )
                .into_response()
        }
        Err(_) => (
            StatusCode::NOT_FOUND,
            axum::Json(json!({ "error": "not_found" })),
        )
            .into_response(),
    }
}

async fn ws_terminal(
    ws: WebSocketUpgrade,
    State(registry): State<Arc<Registry>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_loop(socket, registry, id))
}

async fn ws_loop(mut socket: axum::extract::ws::WebSocket, registry: Arc<Registry>, id: String) {
    use axum::extract::ws::Message;
    let mut last_seq: u64 = 0;
    let mut interval = tokio::time::interval(Duration::from_millis(150));
    loop {
        interval.tick().await;
        let Ok(emulator) = registry.get(&id) else {
            socket
                .send(Message::Text(json!({ "error": "not_found" }).to_string()))
                .await
                .ok();
            return;
        };
        let payload = {
            let st = emulator.state.lock().unwrap();
            if st.grid.seq == last_seq {
                continue;
            }
            last_seq = st.grid.seq;
            json!({
                "id": id,
                "rows": st.grid.rows,
                "cols": st.grid.cols,
                "cursor": { "row": st.grid.cursor_row, "col": st.grid.cursor_col },
                "process_state": st.process_state,
                "text": st.grid.plain_text(),
                "seq": st.grid.seq,
            })
            .to_string()
        };
        if socket.send(Message::Text(payload)).await.is_err() {
            return;
        }
    }
}
