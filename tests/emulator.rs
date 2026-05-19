use std::thread;
use std::time::{Duration, Instant};

use kou_tty::terminal::Emulator;
use kou_tty::terminal::ProcessState;

const MARKER: &str = "kou-tty-integration-ok";

fn wait_for<F: FnMut() -> bool>(timeout: Duration, mut predicate: F) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if predicate() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn echo_through_real_pty_appears_in_grid() {
    let emulator = Emulator::spawn(24, 80, Some("/bin/sh".to_owned())).expect("spawn pty");

    let line = format!("echo {MARKER}\n");
    emulator.write_bytes(line.as_bytes()).expect("write to pty");

    let found = wait_for(Duration::from_secs(3), || {
        let state = emulator.state.lock().unwrap();
        state.grid.plain_text().contains(MARKER)
    });
    assert!(found, "marker '{MARKER}' never appeared in the grid");

    emulator.kill().ok();
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn process_state_reaches_waiting_for_input_after_idle() {
    let emulator = Emulator::spawn(24, 80, Some("/bin/sh".to_owned())).expect("spawn pty");

    // Wait for the prompt to settle.
    let settled = wait_for(Duration::from_secs(3), || {
        let state = emulator.state.lock().unwrap();
        matches!(
            state.process_state,
            ProcessState::Idle | ProcessState::WaitingForInput
        )
    });
    assert!(settled, "process never left the Running state");

    emulator.kill().ok();
}

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn exit_command_marks_terminal_as_exited() {
    let emulator = Emulator::spawn(24, 80, Some("/bin/sh".to_owned())).expect("spawn pty");
    emulator.write_bytes(b"exit\n").expect("write exit");

    let exited = wait_for(Duration::from_secs(3), || {
        let state = emulator.state.lock().unwrap();
        matches!(state.process_state, ProcessState::Exited)
    });
    assert!(exited, "process did not transition to Exited");
}
