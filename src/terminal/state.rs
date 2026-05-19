use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessState {
    Running,
    Idle,
    WaitingForInput,
    Exited,
}

impl Default for ProcessState {
    fn default() -> Self {
        Self::Running
    }
}
