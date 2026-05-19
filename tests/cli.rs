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

fn run(socket: &PathBuf, args: &[&str]) -> Value {
    let output = Command::new(bin())
        .arg("--socket")
        .arg(socket)
        .args(args)
        .output()
        .expect("spawn kou-tty");
    assert!(
        output.status.success(),
        "command {:?} failed: stderr={}",
        args,
        String::from_utf8_lossy(&output.stderr),
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON on stdout")
}

fn run_allow_fail(socket: &PathBuf, args: &[&str]) -> Value {
    let output = Command::new(bin())
        .arg("--socket")
        .arg(socket)
        .args(args)
        .output()
        .expect("spawn kou-tty");
    serde_json::from_slice(&output.stdout).unwrap_or(Value::Null)
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

    let created = run(
        &socket,
        &["create", "--shell", "/bin/sh", "--size", "80x24"],
    );
    let id = id_from(&created);
    assert_eq!(created["ok"], true);

    let sent = run(
        &socket,
        &[
            "send-keys",
            &id,
            r#"[{"text":"echo cli-marker"},{"key":"Enter"}]"#,
        ],
    );
    assert_eq!(sent["ok"], true);

    let mut found = false;
    for _ in 0..30 {
        thread::sleep(Duration::from_millis(100));
        let shown = run(&socket, &["show", &id]);
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

    let status = run(&socket, &["status", &id]);
    assert_eq!(status["result"]["id"].as_str().unwrap(), id);
    assert_eq!(status["result"]["cols"].as_u64().unwrap(), 80);
    assert_eq!(status["result"]["rows"].as_u64().unwrap(), 24);

    let destroyed = run(&socket, &["destroy", &id]);
    assert_eq!(destroyed["ok"], true);

    let _ = run_allow_fail(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn list_returns_active_terminals() {
    let socket = socket_path();
    let created = run(&socket, &["create", "--shell", "/bin/sh"]);
    let id = id_from(&created);

    let listed = run(&socket, &["list"]);
    let terminals = listed["result"]["terminals"]
        .as_array()
        .expect("list array");
    assert!(terminals.iter().any(|t| t["id"] == id));

    run(&socket, &["destroy", &id]);
    let _ = run_allow_fail(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn read_mode_changes_returns_coordinate_overlay() {
    let socket = socket_path();
    let created = run(
        &socket,
        &["create", "--shell", "/bin/sh", "--size", "60x10"],
    );
    let id = id_from(&created);

    run(
        &socket,
        &[
            "send-keys",
            &id,
            r#"[{"text":"printf 'hello\\n'"},{"key":"Enter"}]"#,
        ],
    );
    thread::sleep(Duration::from_millis(300));

    let read = run(&socket, &["read", &id, "--mode", "full"]);
    let text = read["result"]["text"].as_str().expect("text");
    assert!(text.contains("hello"), "expected hello in {text:?}");
    // Coordinate overlay header contains digits 0-9.
    assert!(text.lines().next().unwrap().contains('0'));

    run(&socket, &["destroy", &id]);
    let _ = run_allow_fail(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn resize_changes_size() {
    let socket = socket_path();
    let created = run(&socket, &["create", "--shell", "/bin/sh"]);
    let id = id_from(&created);

    run(&socket, &["resize", &id, "30", "100"]);
    let status = run(&socket, &["status", &id]);
    assert_eq!(status["result"]["rows"].as_u64().unwrap(), 30);
    assert_eq!(status["result"]["cols"].as_u64().unwrap(), 100);

    run(&socket, &["destroy", &id]);
    let _ = run_allow_fail(&socket, &["shutdown"]);
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn unknown_terminal_id_is_a_handled_error() {
    let socket = socket_path();
    let output = Command::new(bin())
        .arg("--socket")
        .arg(&socket)
        .args(["status", "zz"])
        .output()
        .expect("spawn");
    assert!(!output.status.success(), "expected non-zero exit");
    let body: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"]["code"], "not_found");

    let _ = run_allow_fail(&socket, &["shutdown"]);
}
