//! Integration tests for Runner::spawn, driven by the fake_cli bin target
//! so they are hermetic on both ubuntu-latest and windows-latest.

use futures::StreamExt;
use idle_app_lib::contract::{ExitReason, RunEvent, SCRUBBED_ENV_VARS};
use idle_app_lib::engines::RunCtx;
use idle_app_lib::runner::{RunHandle, Runner};
use serial_test::serial;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;

const FAKE_CLI: &str = env!("CARGO_BIN_EXE_fake_cli");

fn ctx(run_id: &str, timeout_secs: u64) -> RunCtx {
    RunCtx {
        run_id: run_id.to_string(),
        cwd: std::env::temp_dir(),
        timeout_secs,
    }
}

fn spawn_fake(mode: &str, ctx: &RunCtx) -> RunHandle {
    Runner::spawn(FAKE_CLI, &[mode], ctx).expect("spawn fake_cli")
}

async fn collect(handle: &mut RunHandle) -> Vec<RunEvent> {
    handle.take_events().collect().await
}

/// The single Output line the fake emits in `env` and `cwd` modes.
fn only_output_line(events: &[RunEvent]) -> String {
    let lines: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            RunEvent::Output { line, .. } => Some(line.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 1, "expected one output event in {events:?}");
    lines[0].to_string()
}

/// Sets the scrubbed API key vars, then restores whatever was there before
/// even if the test panics. All tests in this file are `#[serial]` because
/// `set_var` racing any env read (including `Command::spawn`) is UB.
struct ScrubbedEnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl ScrubbedEnvGuard {
    fn apply() -> Self {
        let saved = SCRUBBED_ENV_VARS
            .iter()
            .copied()
            .map(|var| {
                let previous = std::env::var_os(var);
                std::env::set_var(var, "leaked-from-parent");
                (var, previous)
            })
            .collect();
        Self { saved }
    }
}

impl Drop for ScrubbedEnvGuard {
    fn drop(&mut self) {
        for (var, previous) in self.saved.drain(..) {
            match previous {
                Some(value) => std::env::set_var(var, value),
                None => std::env::remove_var(var),
            }
        }
    }
}

fn process_is_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(windows)]
    {
        process_is_alive_windows(pid)
    }
}

#[cfg(windows)]
fn process_is_alive_windows(pid: u32) -> bool {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        fn GetExitCodeProcess(handle: *mut std::ffi::c_void, exit_code: *mut u32) -> i32;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE
    }
}

async fn wait_until_dead(pid: u32) {
    let start = std::time::Instant::now();
    while process_is_alive(pid) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "child {pid} still alive after handle drop"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
#[serial]
async fn env_scrub_strips_api_keys_and_keeps_path() {
    let _guard = ScrubbedEnvGuard::apply();
    let mut handle = spawn_fake("env", &ctx("r-env", 30));
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Ok);

    let report: serde_json::Value =
        serde_json::from_str(&only_output_line(&events)).expect("env report is JSON");
    assert_eq!(
        report["visible"],
        serde_json::json!([]),
        "child saw scrubbed vars"
    );
    assert_eq!(report["pathPresent"], serde_json::json!(true));
}

#[tokio::test]
#[serial]
async fn child_runs_in_ctx_cwd() {
    let dir = std::env::temp_dir().join("idle-app-runner-cwd-test");
    std::fs::create_dir_all(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();

    let ctx = RunCtx {
        run_id: "r-cwd".to_string(),
        cwd: dir.clone(),
        timeout_secs: 30,
    };
    let mut handle = spawn_fake("cwd", &ctx);
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Ok);

    let report: serde_json::Value =
        serde_json::from_str(&only_output_line(&events)).expect("cwd report is JSON");
    let reported = PathBuf::from(report["cwd"].as_str().unwrap())
        .canonicalize()
        .unwrap();
    assert_eq!(reported, dir);
}

#[tokio::test]
#[serial]
async fn parser_happy_path_stamps_run_id_on_every_event() {
    let mut handle = spawn_fake("emit", &ctx("r-emit", 30));
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Ok);

    let id = || "r-emit".to_string();
    assert_eq!(
        events,
        vec![
            RunEvent::Started { run_id: id() },
            RunEvent::Output {
                run_id: id(),
                line: r#"{"msg":"one"}"#.to_string(),
            },
            RunEvent::Output {
                run_id: id(),
                line: r#"{"msg":"two"}"#.to_string(),
            },
            RunEvent::Output {
                run_id: id(),
                line: r#"{"msg":"three"}"#.to_string(),
            },
            RunEvent::Finished {
                run_id: id(),
                ok: true,
            },
        ]
    );
}

#[tokio::test]
#[serial]
async fn partial_line_across_chunks_parses_as_one_event() {
    let mut handle = spawn_fake("partial", &ctx("r-partial", 30));
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Ok);

    let id = || "r-partial".to_string();
    assert_eq!(
        events,
        vec![
            RunEvent::Started { run_id: id() },
            RunEvent::Output {
                run_id: id(),
                line: r#"{"msg":"split"}"#.to_string(),
            },
            RunEvent::Output {
                run_id: id(),
                line: r#"{"msg":"whole"}"#.to_string(),
            },
            RunEvent::Finished {
                run_id: id(),
                ok: true,
            },
        ]
    );
}

#[tokio::test]
#[serial]
async fn malformed_line_emits_error_and_run_continues() {
    let mut handle = spawn_fake("malformed", &ctx("r-bad", 30));
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Ok);

    let id = || "r-bad".to_string();
    assert_eq!(events[0], RunEvent::Started { run_id: id() });
    assert_eq!(
        events[1],
        RunEvent::Output {
            run_id: id(),
            line: r#"{"msg":"before"}"#.to_string(),
        }
    );
    match &events[2] {
        RunEvent::Error { run_id, message } => {
            assert_eq!(run_id, "r-bad");
            assert!(
                message.contains("this is not json"),
                "raw line missing from {message}"
            );
            assert_ne!(
                message, "this is not json",
                "parse error detail was dropped"
            );
        }
        other => panic!("expected Error, got {other:?}"),
    }
    assert_eq!(
        events[3],
        RunEvent::Output {
            run_id: id(),
            line: r#"{"msg":"after"}"#.to_string(),
        }
    );
    assert_eq!(
        events[4],
        RunEvent::Finished {
            run_id: id(),
            ok: true,
        }
    );
}

#[tokio::test]
#[serial]
async fn timeout_kills_child_and_reports_timeout() {
    let mut handle = spawn_fake("sleep", &ctx("r-timeout", 1));
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Timeout);
    assert_eq!(
        events.last(),
        Some(&RunEvent::Finished {
            run_id: "r-timeout".to_string(),
            ok: false,
        })
    );
}

#[tokio::test]
#[serial]
async fn kill_stops_child_and_reports_cancelled() {
    let mut handle = spawn_fake("sleep", &ctx("r-kill", 60));
    let mut events = handle.take_events();
    assert_eq!(
        events.next().await,
        Some(RunEvent::Started {
            run_id: "r-kill".to_string(),
        })
    );
    handle.kill();
    let rest: Vec<RunEvent> = events.collect().await;
    assert_eq!(handle.wait().await, ExitReason::Cancelled);
    assert_eq!(
        rest.last(),
        Some(&RunEvent::Finished {
            run_id: "r-kill".to_string(),
            ok: false,
        })
    );
}

#[tokio::test]
#[serial]
async fn non_zero_exit_reports_failed_and_surfaces_stderr() {
    let mut handle = spawn_fake("fail", &ctx("r-fail", 30));
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Failed);
    assert_eq!(
        events.last(),
        Some(&RunEvent::Finished {
            run_id: "r-fail".to_string(),
            ok: false,
        })
    );
    assert!(
        events.iter().any(|event| matches!(
            event,
            RunEvent::Error { run_id, message }
                if run_id == "r-fail" && message.contains("something broke")
        )),
        "stderr line missing from {events:?}"
    );
}

#[tokio::test]
#[serial]
async fn timeout_after_stdout_eof_still_fires() {
    let mut handle = spawn_fake("close-stdout-sleep", &ctx("r-eof-timeout", 1));
    let events = tokio::time::timeout(Duration::from_secs(5), collect(&mut handle))
        .await
        .expect("timeout hung after stdout eof");
    assert_eq!(handle.wait().await, ExitReason::Timeout);
    assert_eq!(
        events.last(),
        Some(&RunEvent::Finished {
            run_id: "r-eof-timeout".to_string(),
            ok: false,
        })
    );
}

#[tokio::test]
#[serial]
async fn kill_after_stdout_eof_still_terminates() {
    let mut handle = spawn_fake("close-stdout-sleep", &ctx("r-eof-kill", 60));
    let mut events = handle.take_events();
    assert_eq!(
        events.next().await,
        Some(RunEvent::Started {
            run_id: "r-eof-kill".to_string(),
        })
    );
    tokio::time::sleep(Duration::from_millis(200)).await;
    handle.kill();
    let rest: Vec<RunEvent> = tokio::time::timeout(Duration::from_secs(5), events.collect())
        .await
        .expect("kill hung after stdout eof");
    assert_eq!(handle.wait().await, ExitReason::Cancelled);
    assert_eq!(
        rest.last(),
        Some(&RunEvent::Finished {
            run_id: "r-eof-kill".to_string(),
            ok: false,
        })
    );
}

#[tokio::test]
#[serial]
async fn dropping_handle_stops_the_child() {
    let handle = spawn_fake("sleep", &ctx("r-drop", 60));
    let pid = handle.pid().expect("child pid");
    assert!(process_is_alive(pid), "child {pid} was not running");
    drop(handle);
    wait_until_dead(pid).await;
}

#[tokio::test]
#[serial]
async fn burst_exit_drains_every_stdout_line() {
    let expected = vec![
        r#"{"n":1}"#,
        r#"{"n":2}"#,
        r#"{"n":3}"#,
        r#"{"n":4}"#,
        r#"{"n":5}"#,
    ];
    for i in 0..50 {
        let run_id = format!("r-burst-{i}");
        let mut handle = spawn_fake("burst", &ctx(&run_id, 30));
        let events = collect(&mut handle).await;
        assert_eq!(handle.wait().await, ExitReason::Ok, "iteration {i}");
        assert_eq!(
            events.first(),
            Some(&RunEvent::Started {
                run_id: run_id.clone(),
            }),
            "iteration {i}"
        );
        let lines: Vec<&str> = events
            .iter()
            .filter_map(|event| match event {
                RunEvent::Output { line, .. } => Some(line.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(lines, expected, "iteration {i} dropped stdout: {events:?}");
        assert_eq!(
            events.last(),
            Some(&RunEvent::Finished { run_id, ok: true }),
            "iteration {i}"
        );
    }
}
