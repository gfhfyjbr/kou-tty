use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::protocol::{ApiResponse, Request};

pub struct DaemonClient {
    socket: PathBuf,
}

impl DaemonClient {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    pub async fn send(&self, request: &Request) -> Result<ApiResponse> {
        self.ensure_daemon().await?;
        let stream = UnixStream::connect(&self.socket)
            .await
            .with_context(|| format!("connect {}", self.socket.display()))?;
        let (reader, mut writer) = stream.into_split();
        let mut line = serde_json::to_vec(request)?;
        line.push(b'\n');
        writer.write_all(&line).await?;
        writer.shutdown().await.ok();
        let mut lines = BufReader::new(reader).lines();
        let response_line = lines
            .next_line()
            .await?
            .context("daemon closed connection without response")?;
        let response: ApiResponse = serde_json::from_str(&response_line)?;
        Ok(response)
    }

    pub async fn ensure_daemon(&self) -> Result<()> {
        if UnixStream::connect(&self.socket).await.is_ok() {
            return Ok(());
        }
        spawn_daemon(&self.socket)?;
        wait_for_daemon(&self.socket).await
    }
}

fn spawn_daemon(socket: &Path) -> Result<()> {
    let exe = std::env::current_exe().context("locate current_exe")?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("daemon").arg("--socket").arg(socket);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc_setsid();
                Ok(())
            });
        }
    }
    cmd.spawn().context("spawn daemon")?;
    Ok(())
}

#[cfg(unix)]
fn libc_setsid() {
    unsafe extern "C" {
        fn setsid() -> i32;
    }
    unsafe {
        setsid();
    }
}

async fn wait_for_daemon(socket: &Path) -> Result<()> {
    for _ in 0..50 {
        if UnixStream::connect(socket).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("daemon did not start within timeout");
}
