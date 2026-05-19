use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum Request {
    Ping,
    TerminalCreate {
        #[serde(default)]
        size: Option<Size>,
        #[serde(default)]
        shell: Option<String>,
    },
    TerminalDestroy {
        id: String,
    },
    TerminalList,
    TerminalSendKey {
        id: String,
        key: String,
    },
    TerminalSendKeys {
        id: String,
        input: Vec<KeyInput>,
    },
    TerminalMouse {
        id: String,
        #[serde(flatten)]
        action: MouseAction,
    },
    TerminalReadScreen {
        id: String,
        #[serde(default)]
        mode: ReadMode,
        #[serde(default)]
        max_lines: Option<u16>,
    },
    TerminalShowScreen {
        id: String,
    },
    TerminalReadRows {
        id: String,
        from: u16,
        to: u16,
    },
    TerminalReadRegion {
        id: String,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    },
    TerminalStatus {
        id: String,
    },
    TerminalPollEvents {
        id: String,
        #[serde(default)]
        max: Option<usize>,
    },
    TerminalSelect {
        id: String,
        from_row: u16,
        from_col: u16,
        to_row: u16,
        to_col: u16,
    },
    TerminalScroll {
        id: String,
        by: i32,
    },
    TerminalResize {
        id: String,
        rows: u16,
        cols: u16,
    },
    ViewerStart {
        #[serde(default)]
        port: Option<u16>,
    },
    ViewerStop,
    ViewerStatus,
    Shutdown,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Size {
    Named(String),
    Explicit { rows: u16, cols: u16 },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum KeyInput {
    Text { text: String },
    Key { key: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum MouseAction {
    Click {
        button: String,
        x: u16,
        y: u16,
    },
    Press {
        button: String,
        x: u16,
        y: u16,
    },
    Release {
        button: String,
        x: u16,
        y: u16,
    },
    Scroll {
        direction: String,
        x: u16,
        y: u16,
    },
    Drag {
        from_x: u16,
        from_y: u16,
        to_x: u16,
        to_y: u16,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadMode {
    #[default]
    Full,
    Changes,
    Plain,
}
