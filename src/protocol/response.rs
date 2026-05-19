use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl ApiResponse {
    pub fn ok(result: Value) -> Self {
        Self {
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                suggestion: None,
            }),
        }
    }

    pub fn err_hint(
        code: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            ok: false,
            result: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
                suggestion: Some(suggestion.into()),
            }),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Maps an error code to a POSIX-friendly exit code that agents can branch on.
pub fn exit_code_for(response: &ApiResponse) -> i32 {
    if response.ok {
        return 0;
    }
    let Some(err) = &response.error else {
        return 1;
    };
    match err.code.as_str() {
        "bad_request" | "bad_size" | "bad_key" | "bad_input" => 2,
        "not_found" => 3,
        "conflict" | "already_exists" => 5,
        "shutdown" => 0,
        _ => 1,
    }
}
