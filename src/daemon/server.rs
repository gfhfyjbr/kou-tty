use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::protocol::{ApiResponse, Request};

use super::handler::{DaemonContext, handle_request};

pub fn default_socket_path() -> PathBuf {
    let runtime_dir = if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir)
    } else {
        std::env::temp_dir()
    };
    let uid = libc_getuid();
    runtime_dir.join(format!("kou-tty-{uid}.sock"))
}

#[cfg(unix)]
fn libc_getuid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}

#[cfg(not(unix))]
fn libc_getuid() -> u32 {
    0
}

pub async fn run(socket_path: &Path) -> Result<()> {
    if socket_path.exists() {
        if probe_existing(socket_path).await {
            anyhow::bail!(
                "another kou-tty daemon is already running on {}",
                socket_path.display()
            );
        }
        std::fs::remove_file(socket_path).ok();
    }
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let listener = UnixListener::bind(socket_path)
        .with_context(|| format!("bind {}", socket_path.display()))?;
    tracing::info!(socket = %socket_path.display(), "kou-tty daemon listening");

    let ctx = DaemonContext::new();
    let shutdown_ctx = Arc::clone(&ctx);

    loop {
        tokio::select! {
            _ = shutdown_ctx.shutdown.notified() => {
                tracing::info!("shutdown requested");
                break;
            }
            accept = listener.accept() => {
                match accept {
                    Ok((stream, _addr)) => {
                        let conn_ctx = Arc::clone(&ctx);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(conn_ctx, stream).await {
                                tracing::debug!(error = %e, "connection error");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "accept failed");
                    }
                }
            }
        }
    }

    std::fs::remove_file(socket_path).ok();
    Ok(())
}

async fn handle_connection(ctx: Arc<DaemonContext>, stream: UnixStream) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle_request(Arc::clone(&ctx), req).await,
            Err(e) => ApiResponse::err("bad_request", e.to_string()),
        };
        let mut out = serde_json::to_vec(&response).unwrap_or_else(|_| b"{}".to_vec());
        out.push(b'\n');
        if writer.write_all(&out).await.is_err() {
            break;
        }
    }
    Ok(())
}

async fn probe_existing(path: &Path) -> bool {
    match UnixStream::connect(path).await {
        Ok(mut stream) => {
            let probe = b"{\"method\":\"ping\"}\n";
            if stream.write_all(probe).await.is_err() {
                return false;
            }
            let mut buf = [0u8; 256];
            tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
                .await
                .map(|n| n > 0)
                .unwrap_or(false)
        }
        Err(_) => false,
    }
}
