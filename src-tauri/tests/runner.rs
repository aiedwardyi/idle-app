//! Integration tests for Runner::spawn, driven by the fake_cli bin target
//! so they are hermetic on both ubuntu-latest and windows-latest.

use futures::StreamExt;
use idle_app_lib::contract::{ExitReason, RunEvent, SCRUBBED_ENV_VARS};
use idle_app_lib::engines::RunCtx;
use idle_app_lib::runner::{RunHandle, Runner};
use std::path::PathBuf;

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

#[tokio::test]
async fn env_scrub_strips_api_keys_and_keeps_path() {
    for var in SCRUBBED_ENV_VARS {
        std::env::set_var(var, "leaked-from-parent");
    }
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
async fn malformed_line_emits_error_and_run_continues() {
    let mut handle = spawn_fake("malformed", &ctx("r-bad", 30));
    let events = collect(&mut handle).await;
    assert_eq!(handle.wait().await, ExitReason::Ok);

    let id = || "r-bad".to_string();
    assert_eq!(
        events,
        vec![
            RunEvent::Started { run_id: id() },
            RunEvent::Output {
                run_id: id(),
                line: r#"{"msg":"before"}"#.to_string(),
            },
            RunEvent::Error {
                run_id: id(),
                message: "this is not json".to_string(),
            },
            RunEvent::Output {
                run_id: id(),
                line: r#"{"msg":"after"}"#.to_string(),
            },
            RunEvent::Finished {
                run_id: id(),
                ok: true,
            },
        ]
    );
}

#[tokio::test]
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
