//! Hermetic fake CLI for runner tests.
//!
//! Built as a second bin target and invoked from tests via
//! `env!("CARGO_BIN_EXE_fake_cli")`, so the tests pass on both
//! ubuntu-latest and windows-latest without any shell scripts.
//! Spawns no subprocesses itself.

use idle_app_lib::contract::SCRUBBED_ENV_VARS;
use std::io::Write;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        "emit" => emit(),
        "malformed" => malformed(),
        "partial" => partial(),
        "sleep" => sleep_forever(),
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
