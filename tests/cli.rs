use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::Value;

static SOCKET_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_kou-tty")
}

fn socket_path() -> PathBuf {
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let n = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    p.push(format!("kou-tty-cli-test-{pid}-{n}.sock"));
    let _ = std::fs::remove_file(&p);
    p
}

/// Run a subcommand asking for JSON output and assert success.
fn run_json(socket: &PathBuf, args: &[&str]) -> Value {
    let mut full: Vec<&str> = vec!["--json"];
    full.extend(args.iter().copied());
    let output = Command::new(bin())
        .arg("--socket")
        .arg(socket)
        .args(&full)
        .output()
        .expect("spawn kou-tty");
    assert!(
        output.status.success(),
        "command {:?} failed (exit {}): stderr={}",
        args,
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr),
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON on stdout")
}

/// Run a subcommand and capture exit code, parsed JSON (if any), and stderr.
fn run_capture(socket: &PathBuf, args: &[&str]) -> (i32, Value, String) {
    let mut full: Vec<&str> = vec!["--json"];
    full.extend(args.iter().copied());
    let output = Command::new(bin())
        .arg("--socket")
        .arg(socket)
        .args(&full)
        .output()
        .expect("spawn kou-tty");
    let body: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code().unwrap_or(-1);
    (code, body, stderr)
}

/// Run a subcommand in default (bare) mode and return trimmed stdout.
fn bare_stdout(socket: &PathBuf, args: &[&str]) -> String {
    let output = Command::new(bin())
        .arg("--socket")
        .arg(socket)
        .args(args)
        .output()
        .expect("spawn kou-tty");
    assert!(
        output.status.success(),
        "command {:?} exited {}: {}",
        args,
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn id_from(create: &Value) -> String {
    create["result"]["id"]
        .as_str()
        .expect("id in create result")
        .to_owned()
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn create_send_read_destroy_cycle() {
    let socket = socket_path();

    let created = run_json(
        &socket,
        &[
            "terminal", "create", "--shell", "/bin/sh", "--size", "80x24",
        ],
    );
    let id = id_from(&created);
    assert_eq!(created["ok"], true);

    let sent = run_json(
        &socket,
        &[
            "terminal",
            "send-keys",
            &id,
            r#"[{"text":"echo cli-marker"},{"key":"Enter"}]"#,
        ],
    );
    assert_eq!(sent["ok"], true);

    let mut found = false;
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(100));
        let shown = run_json(&socket, &["terminal", "show", &id]);
        if shown["result"]["text"]
            .as_str()
            .map(|t| t.contains("cli-marker"))
            .unwrap_or(false)
        {
            found = true;
            break;
        }
    }
    assert!(found, "marker never appeared in show output");

    let status = run_json(&socket, &["terminal", "status", &id]);
    assert_eq!(status["result"]["id"].as_str().unwrap(), id);
    assert_eq!(status["result"]["cols"].as_u64().unwrap(), 80);
    assert_eq!(status["result"]["rows"].as_u64().unwrap(), 24);

    let destroyed = run_json(&socket, &["terminal", "destroy", &id]);
    assert_eq!(destroyed["ok"], true);

    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn list_returns_active_terminals() {
    let socket = socket_path();
    let created = run_json(&socket, &["terminal", "create", "--shell", "/bin/sh"]);
    let id = id_from(&created);

    let listed = run_json(&socket, &["terminal", "list"]);
    let terminals = listed["result"]["terminals"]
        .as_array()
        .expect("list array");
    assert!(terminals.iter().any(|t| t["id"] == id));

    run_json(&socket, &["terminal", "destroy", &id]);
    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn read_full_returns_coordinate_overlay() {
    let socket = socket_path();
    let created = run_json(
        &socket,
        &[
            "terminal", "create", "--shell", "/bin/sh", "--size", "60x10",
        ],
    );
    let id = id_from(&created);

    run_json(
        &socket,
        &[
            "terminal",
            "send-keys",
            &id,
            r#"[{"text":"printf 'hello\\n'"},{"key":"Enter"}]"#,
        ],
    );
    thread::sleep(Duration::from_millis(300));

    let read = run_json(&socket, &["terminal", "read", &id, "--mode", "full"]);
    let text = read["result"]["text"].as_str().expect("text");
    assert!(text.contains("hello"), "expected hello in {text:?}");
    assert!(text.lines().next().unwrap().contains('0'));

    run_json(&socket, &["terminal", "destroy", &id]);
    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn resize_changes_size() {
    let socket = socket_path();
    let created = run_json(&socket, &["terminal", "create", "--shell", "/bin/sh"]);
    let id = id_from(&created);

    run_json(&socket, &["terminal", "resize", &id, "30", "100"]);
    let status = run_json(&socket, &["terminal", "status", &id]);
    assert_eq!(status["result"]["rows"].as_u64().unwrap(), 30);
    assert_eq!(status["result"]["cols"].as_u64().unwrap(), 100);

    run_json(&socket, &["terminal", "destroy", &id]);
    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn unknown_id_returns_exit_code_3_with_suggestion() {
    let socket = socket_path();
    let (code, body, stderr) = run_capture(&socket, &["terminal", "status", "zz"]);
    assert_eq!(code, 3, "expected exit 3 for not_found, got {code}");
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(body["error"]["suggestion"].is_string());
    assert!(stderr.contains("error[not_found]"));

    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn destroy_if_exists_is_idempotent() {
    let socket = socket_path();
    let (code, _body, _) = run_capture(&socket, &["terminal", "destroy", "zz"]);
    assert_eq!(code, 3, "without --if-exists should fail with 3");

    let (code, body, _) = run_capture(&socket, &["terminal", "destroy", "zz", "--if-exists"]);
    assert_eq!(code, 0, "with --if-exists should succeed");
    assert_eq!(body["ok"], true);
    assert_eq!(body["result"]["missing"], true);

    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn shutdown_returns_exit_zero() {
    let socket = socket_path();
    run_json(&socket, &["terminal", "create", "--shell", "/bin/sh"]);
    let (code, body, _) = run_capture(&socket, &["shutdown"]);
    assert_eq!(code, 0);
    assert_eq!(body["ok"], true);
    assert!(
        body["result"]
            .as_object()
            .unwrap()
            .get("__shutdown__")
            .is_none()
    );
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn default_create_prints_only_the_id() {
    let socket = socket_path();
    let id = bare_stdout(&socket, &["terminal", "create", "--shell", "/bin/sh"]);
    assert!(!id.is_empty());
    assert!(!id.contains('{'), "expected bare id, got {id:?}");
    assert!(id.len() == 2, "expected 2-char id, got {id:?}");

    run_json(&socket, &["terminal", "destroy", &id]);
    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn default_send_keys_prints_nothing() {
    let socket = socket_path();
    let id = bare_stdout(&socket, &["terminal", "create", "--shell", "/bin/sh"]);
    let stdout = bare_stdout(
        &socket,
        &[
            "terminal",
            "send-keys",
            &id,
            r#"[{"text":"echo silence"},{"key":"Enter"}]"#,
        ],
    );
    assert!(stdout.is_empty(), "expected empty stdout, got {stdout:?}");

    run_json(&socket, &["terminal", "destroy", &id]);
    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn default_status_prints_process_state() {
    let socket = socket_path();
    let id = bare_stdout(&socket, &["terminal", "create", "--shell", "/bin/sh"]);
    let state = bare_stdout(&socket, &["terminal", "status", &id]);
    assert!(
        matches!(state.as_str(), "running" | "idle" | "waiting_for_input"),
        "unexpected state {state:?}"
    );

    run_json(&socket, &["terminal", "destroy", &id]);
    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn compact_emits_single_line_json() {
    let socket = socket_path();
    let body = bare_stdout(
        &socket,
        &["--compact", "terminal", "create", "--shell", "/bin/sh"],
    );
    assert!(!body.contains('\n'), "expected single line, got {body:?}");
    let parsed: Value = serde_json::from_str(&body).expect("valid json");
    assert_eq!(parsed["ok"], true);

    let id = parsed["result"]["id"].as_str().unwrap();
    run_json(&socket, &["terminal", "destroy", id]);
    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn default_list_prints_one_id_per_line() {
    let socket = socket_path();
    bare_stdout(&socket, &["terminal", "create", "--shell", "/bin/sh"]);
    bare_stdout(&socket, &["terminal", "create", "--shell", "/bin/sh"]);

    let listed = bare_stdout(&socket, &["terminal", "list"]);
    let ids: Vec<&str> = listed.lines().collect();
    assert_eq!(ids.len(), 2);
    for id in &ids {
        assert_eq!(id.len(), 2, "expected 2-char id, got {id:?}");
    }

    let _ = run_capture(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn error_writes_to_stderr_and_stdout_stays_empty_in_bare_mode() {
    let socket = socket_path();
    let output = Command::new(bin())
        .arg("--socket")
        .arg(&socket)
        .args(["terminal", "status", "zz"])
        .output()
        .expect("spawn");
    assert_eq!(output.status.code(), Some(3));
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout, got {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[not_found]"));
    assert!(stderr.contains("hint:"));

    let _ = run_capture(&socket, &["shutdown"]);
}
