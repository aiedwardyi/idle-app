//! Hermetic fake CLI for runner and adapter tests.
//!
//! Built as a second bin target and invoked from tests via
//! `env!("CARGO_BIN_EXE_fake_cli")`, so the tests pass on both
//! ubuntu-latest and windows-latest without any shell scripts.
//! One mode spawns a grandchild to hold the stdout pipe; the rest do not.
//!
//! It also stands in for the `claude` binary: when the first argument is a
//! claude flag or subcommand (`--version`, `auth`, `-p`), it answers the way
//! the real CLI does, replaying fixture files on request. The adapter under
//! test never knows the difference.

use idle_app_lib::contract::SCRUBBED_ENV_VARS;
use std::io::Write;
use std::process::Stdio;

/// Path of the `auth status` fixture to print. Exit code follows the
/// `loggedIn` value in the file, like the real CLI.
const AUTH_FIXTURE_ENV: &str = "IDLE_FAKE_CLI_AUTH_FIXTURE";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => return claude_version(),
        Some("auth") => return claude_auth_status(),
        Some("-p") => return claude_print_run(&args),
        _ => {}
    }
    let mode = args.first().cloned().unwrap_or_default();
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

/// `claude --version` prints one plain text line, not JSON.
fn claude_version() {
    say("9.8.7 (Claude Code)");
}

/// `claude auth status` prints pretty JSON and exits 1 when signed out.
fn claude_auth_status() {
    let Some(path) = std::env::var_os(AUTH_FIXTURE_ENV) else {
        eprintln!("{AUTH_FIXTURE_ENV} not set");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(&path).expect("read auth fixture");
    print!("{text}");
    let _ = std::io::stdout().flush();
    // Parse rather than string-match so a reformatted fixture cannot flip
    // the exit code silently; a fixture without the field fails loudly.
    let status: serde_json::Value = serde_json::from_str(&text).expect("auth fixture is JSON");
    let logged_in = status["loggedIn"]
        .as_bool()
        .expect("auth fixture has a boolean loggedIn");
    if !logged_in {
        std::process::exit(1);
    }
}

/// `claude -p ... -- <prompt>`. The prompt selects the behaviour so tests
/// need no env vars and can run in parallel:
///
/// - `replay <exit_code> <path>` prints the fixture file line by line and
///   exits with `exit_code`, standing in for a real stream-json run.
/// - `hang` prints one init line and never exits, for kill tests.
/// - anything else echoes the full argv as a `system` line so a test can
///   prove the prompt survived the trip through the OS.
fn claude_print_run(args: &[String]) {
    let prompt = args
        .iter()
        .position(|arg| arg == "--")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();
    if prompt == "hang" {
        say(r#"{"type":"system","subtype":"init","session_id":"fake"}"#);
        sleep_forever();
    }
    if let Some(rest) = prompt.strip_prefix("replay ") {
        let (code, path) = rest.split_once(' ').expect("replay <exit_code> <path>");
        let text = std::fs::read_to_string(path).expect("read replay fixture");
        let mut out = std::io::stdout();
        for line in text.lines() {
            writeln!(out, "{line}").unwrap();
        }
        out.flush().unwrap();
        std::process::exit(code.parse().expect("exit code"));
    }
    println!(
        "{}",
        serde_json::json!({ "type": "system", "subtype": "fake_argv", "args": args })
    );
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
