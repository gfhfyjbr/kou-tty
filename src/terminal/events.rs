use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::state::ProcessState;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TerminalEvent {
    CommandFinished {
        exit_code: Option<i32>,
        timestamp_ms: u128,
    },
    WaitingForInput {
        timestamp_ms: u128,
    },
    Bell,
    ProcessStateChanged {
        from: ProcessState,
        to: ProcessState,
        timestamp_ms: u128,
    },
    ScreenChanged {
        rows: Vec<u16>,
        timestamp_ms: u128,
    },
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
