//! Hermetic fake CLI for runner tests.
//!
//! Built as a second bin target and invoked from tests via
//! `env!("CARGO_BIN_EXE_fake_cli")`, so the tests pass on both
//! ubuntu-latest and windows-latest without any shell scripts.
//! One mode spawns a grandchild to hold the stdout pipe; the rest do not.

use idle_app_lib::contract::SCRUBBED_ENV_VARS;
use std::io::Write;
use std::process::Stdio;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "emit" => emit(),
        "burst" => burst(),
        "malformed" => malformed(),
        "partial" => partial(),
        "sleep" => sleep_forever(),
        "close-stdout-sleep" => close_stdout_then_sleep(),
        "hold-stdout" => hold_stdout_grandchild(),
        "hold-stdout-exit" => hold_stdout_then_exit(),
        "hold-pipe" => hold_pipe(),
        "fail" => fail(),
        "env" => report_env(),
        "cwd" => report_cwd(),
        other => {
            eprintln!("unknown mode: {other}");
            std::process::exit(2);
        }
    }
}

fn say(line: &str) {
    println!("{line}");
}

/// A valid JSON-lines stream, then exit 0.
fn emit() {
    say(r#"{"msg":"one"}"#);
    say(r#"{"msg":"two"}"#);
    say(r#"{"msg":"three"}"#);
}

/// Several JSON lines in one write, then exit with no delay. Used to
/// catch a race where child.wait() is observed before stdout is drained.
fn burst() {
    let mut out = std::io::stdout();
    out.write_all(b"{\"n\":1}\n{\"n\":2}\n{\"n\":3}\n{\"n\":4}\n{\"n\":5}\n")
        .unwrap();
    out.flush().unwrap();
}

/// A valid line, a malformed line, then another valid line.
fn malformed() {
    say(r#"{"msg":"before"}"#);
    say("this is not json");
    say(r#"{"msg":"after"}"#);
}

/// One JSON object split across two flushed stdout chunks, then a whole
/// line. Exercises the reader's line buffering.
fn partial() {
    let mut out = std::io::stdout();
    out.write_all(br#"{"msg":"spl"#).unwrap();
    out.flush().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    out.write_all(b"it\"}\n").unwrap();
    out.write_all(b"{\"msg\":\"whole\"}\n").unwrap();
}

/// Never exits. The runner's timeout or kill has to stop it.
fn sleep_forever() {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

/// Close stdout so the parent sees EOF, then block. Used to prove the
/// runner still honors timeout and kill after stdout ends.
fn close_stdout_then_sleep() {
    let _ = std::io::stdout().flush();
    close_stdout();
    sleep_forever();
}

fn close_stdout() {
    #[cfg(unix)]
    {
        use std::os::fd::FromRawFd;
        // Safety: fd 1 is stdout. Dropping the owned fd closes it so the
        // parent's pipe sees EOF while this process keeps running.
        drop(unsafe { std::fs::File::from_raw_fd(1) });
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        let raw = std::io::stdout().as_raw_handle();
        // Safety: take ownership of the current stdout handle so drop
        // closes the pipe to the parent.
        drop(unsafe { OwnedHandle::from_raw_handle(raw) });
    }
}

/// Parent sleeps after spawning a grandchild that inherits stdout.
/// Killing this process leaves the pipe open until the grandchild exits.
fn hold_stdout_grandchild() {
    spawn_stdout_holder();
    say(r#"{"msg":"parent"}"#);
    sleep_forever();
}

/// Parent exits 0 after spawning a grandchild that inherits stdout.
/// The pipe stays open even though the direct child reported success.
fn hold_stdout_then_exit() {
    spawn_stdout_holder();
    say(r#"{"msg":"parent"}"#);
}

fn spawn_stdout_holder() {
    let exe = std::env::current_exe().expect("current exe");
    // Test fixture only. This binary is the fake CLI, not idle-app.
    // Do not wait: the grandchild must outlive this process.
    #[allow(clippy::zombie_processes)]
    let _grandchild = std::process::Command::new(exe)
        .arg("hold-pipe")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn grandchild");
}

/// Grandchild: keep the inherited stdout write-end open. Exit when the
/// read end is gone so tests do not leak processes.
fn hold_pipe() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let mut out = std::io::stdout();
        if out.write_all(b"{\"hold\":true}\n").is_err() {
            break;
        }
        let _ = out.flush();
    }
}

/// Writes to stderr and exits non-zero.
fn fail() {
    eprintln!("something broke");
    std::process::exit(3);
}

/// Reports which scrubbed env vars are visible plus whether PATH survived.
fn report_env() {
    let visible: Vec<&str> = SCRUBBED_ENV_VARS
        .iter()
        .copied()
        .filter(|var| std::env::var_os(var).is_some())
        .collect();
    let path_present = std::env::var_os("PATH").is_some();
    println!(
        "{}",
        serde_json::json!({ "visible": visible, "pathPresent": path_present })
    );
}

/// Reports the cwd this process was launched with.
fn report_cwd() {
    let cwd = std::env::current_dir().unwrap();
    println!("{}", serde_json::json!({ "cwd": cwd }));
}
