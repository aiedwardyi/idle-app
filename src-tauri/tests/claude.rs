//! Hermetic tests for the Claude adapter. `fake_cli` stands in for the
//! `claude` binary and replays the fixtures under `tests/fixtures/claude`,
//! so nothing here needs a real install or a network.
//!
//! The one exception is the `#[ignore]` live test at the bottom, which runs
//! a real task through the real binary and prints every event:
//!
//!     cargo test --test claude live_run -- --ignored --nocapture

use futures::StreamExt;
use idle_app_lib::contract::{
    default_windows, DetectInfo, EngineChoice, EngineId, ExitReason, RunEvent, Task, TaskSize,
    TaskStatus,
};
use idle_app_lib::engines::claude::ClaudeEngine;
use idle_app_lib::engines::{Engine, EngineError, EngineRun, RunCtx};
use serial_test::serial;
use std::path::PathBuf;
use std::time::Duration;

const FAKE_CLI: &str = env!("CARGO_BIN_EXE_fake_cli");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(name)
}

fn task(prompt: &str) -> Task {
    Task {
        id: "task-1".into(),
        prompt: prompt.into(),
        folder: std::env::temp_dir().to_string_lossy().into_owned(),
        size: TaskSize::S,
        engine: EngineChoice::Fixed(EngineId::Claude),
        status: TaskStatus::Queued,
        created_at: "2026-09-03T00:00:00Z".into(),
        updated_at: "2026-09-03T00:00:00Z".into(),
    }
}

fn ctx(run_id: &str) -> RunCtx {
    RunCtx {
        run_id: run_id.into(),
        cwd: std::env::temp_dir(),
        timeout_secs: 30,
    }
}

fn fake_engine() -> ClaudeEngine {
    ClaudeEngine::with_program(FAKE_CLI)
}

/// Replay a fixture through the adapter and collect the whole stream.
async fn replay(run_id: &str, exit_code: i32, name: &str) -> (Vec<RunEvent>, ExitReason) {
    let prompt = format!("replay {exit_code} {}", fixture(name).display());
    let mut run = fake_engine()
        .run(&task(&prompt), ctx(run_id))
        .expect("spawn fake claude");
    collect(&mut run).await
}

async fn collect(run: &mut EngineRun) -> (Vec<RunEvent>, ExitReason) {
    let events: Vec<RunEvent> = run.take_events().collect().await;
    let reason = run.wait().await;
    (events, reason)
}

fn run_id_of(event: &RunEvent) -> &str {
    match event {
        RunEvent::Started { run_id }
        | RunEvent::Output { run_id, .. }
        | RunEvent::Usage { run_id, .. }
        | RunEvent::LimitHit { run_id, .. }
        | RunEvent::Finished { run_id, .. }
        | RunEvent::Error { run_id, .. } => run_id,
    }
}

fn count(events: &[RunEvent], pred: impl Fn(&RunEvent) -> bool) -> usize {
    events.iter().filter(|e| pred(e)).count()
}

fn errors(events: &[RunEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|e| match e {
            RunEvent::Error { message, .. } => Some(message.as_str()),
            _ => None,
        })
        .collect()
}

/// Shape every run must have: Started first, exactly one Finished last,
/// and the run id stamped on every event.
fn assert_stream_shape(events: &[RunEvent], run_id: &str) {
    assert!(
        matches!(events.first(), Some(RunEvent::Started { .. })),
        "first event must be Started: {events:?}"
    );
    assert!(
        matches!(events.last(), Some(RunEvent::Finished { .. })),
        "last event must be Finished: {events:?}"
    );
    assert_eq!(
        count(events, |e| matches!(e, RunEvent::Finished { .. })),
        1,
        "exactly one Finished: {events:?}"
    );
    for event in events {
        assert_eq!(run_id_of(event), run_id, "missing run id on {event:?}");
    }
}

#[tokio::test]
async fn success_run_emits_usage_exactly_once_from_result_total() {
    let (events, reason) = replay("r-ok", 0, "run_success.jsonl").await;
    assert_stream_shape(&events, "r-ok");
    assert_eq!(reason, ExitReason::Ok);
    assert_eq!(
        events.last(),
        Some(&RunEvent::Finished {
            run_id: "r-ok".into(),
            ok: true,
        })
    );

    // Invariant 4: one Usage event, equal to result.usage. The per-message
    // assistant usage (17 + 1 output tokens) must not be added on top of the
    // result total (118), and cache is creation + read.
    let usages: Vec<&RunEvent> = events
        .iter()
        .filter(|e| matches!(e, RunEvent::Usage { .. }))
        .collect();
    assert_eq!(usages.len(), 1, "usage must be emitted once: {events:?}");
    assert_eq!(
        usages[0],
        &RunEvent::Usage {
            run_id: "r-ok".into(),
            input: 4,
            output: 118,
            cache: 21986 + 45376,
        }
    );

    assert_eq!(
        errors(&events),
        Vec::<&str>::new(),
        "clean run has no errors"
    );
    assert_eq!(
        count(&events, |e| matches!(e, RunEvent::LimitHit { .. })),
        0
    );
    // Every recognised line is passed through verbatim as Output.
    assert_eq!(
        count(&events, |e| matches!(e, RunEvent::Output { .. })),
        14,
        "all 14 fixture lines reach the caller"
    );
}

#[tokio::test]
async fn unrecognized_event_type_surfaces_as_error_with_raw_line() {
    let (events, reason) = replay("r-unknown", 0, "run_unrecognized_event.jsonl").await;
    assert_stream_shape(&events, "r-unknown");
    assert_eq!(
        reason,
        ExitReason::Ok,
        "an unknown event does not fail the run"
    );

    let errs = errors(&events);
    assert_eq!(errs.len(), 1, "exactly one error: {events:?}");
    assert!(
        errs[0].contains(r#""type":"usage_forecast""#),
        "raw line must be on the error: {}",
        errs[0]
    );
    // Recognised lines still flow: 14 known lines, the odd one is the Error.
    assert_eq!(count(&events, |e| matches!(e, RunEvent::Output { .. })), 14);
    assert_eq!(count(&events, |e| matches!(e, RunEvent::Usage { .. })), 1);
}

#[tokio::test]
async fn limit_hit_is_limit_hit_then_finished_never_error() {
    // The real CLI exits 1 after a limit rejection.
    let (events, reason) = replay("r-limit", 1, "run_limit_hit.synthetic.jsonl").await;
    assert_stream_shape(&events, "r-limit");

    // Invariant 5: the terminal pair is LimitHit then Finished, the exit
    // reason is LimitHit, and no Error event is raised for the limit.
    assert_eq!(reason, ExitReason::LimitHit);
    let n = events.len();
    assert_eq!(
        events[n - 2],
        RunEvent::LimitHit {
            run_id: "r-limit".into(),
            resets_at: Some("2026-09-03T02:50:00Z".into()),
        },
        "LimitHit must directly precede Finished: {events:?}"
    );
    assert_eq!(
        events[n - 1],
        RunEvent::Finished {
            run_id: "r-limit".into(),
            ok: false,
        }
    );
    assert_eq!(
        errors(&events),
        Vec::<&str>::new(),
        "a limit is not an error"
    );
    // Zero usage is still the run's true total, reported once.
    assert_eq!(
        count(&events, |e| matches!(
            e,
            RunEvent::Usage {
                input: 0,
                output: 0,
                cache: 0,
                ..
            }
        )),
        1
    );
}

#[tokio::test]
async fn error_result_is_failed_even_when_exit_code_is_zero() {
    let (events, reason) = replay("r-err", 0, "run_error_result.jsonl").await;
    assert_stream_shape(&events, "r-err");
    assert_eq!(
        reason,
        ExitReason::Failed,
        "result.is_error overrides exit 0"
    );
    assert_eq!(
        events.last(),
        Some(&RunEvent::Finished {
            run_id: "r-err".into(),
            ok: false,
        })
    );
    let errs = errors(&events);
    assert_eq!(errs.len(), 1, "{events:?}");
    assert!(
        errs[0].contains("bogus-model-xyz"),
        "the CLI's own error text reaches the caller: {}",
        errs[0]
    );
    assert_eq!(
        count(&events, |e| matches!(e, RunEvent::LimitHit { .. })),
        0,
        "a model error is not a limit"
    );
}

#[tokio::test]
async fn prompt_with_quotes_newlines_and_10k_chars_survives_argv() {
    let mut prompt = String::from("Fix the \"quoted\" bug.\nSecond line with 'single' quotes.\n");
    prompt.push_str("-starts with a dash so the CLI must not read it as a flag\n");
    while prompt.len() < 10_000 {
        prompt.push_str("padding \"with quotes\" and\nnewlines ");
    }
    let mut run = fake_engine()
        .run(&task(&prompt), ctx("r-argv"))
        .expect("spawn fake claude");
    let (events, reason) = collect(&mut run).await;
    assert_stream_shape(&events, "r-argv");
    assert_eq!(reason, ExitReason::Ok);

    let echoed = events
        .iter()
        .find_map(|e| match e {
            RunEvent::Output { line, .. } if line.contains("fake_argv") => Some(line),
            _ => None,
        })
        .expect("argv echo line");
    let value: serde_json::Value = serde_json::from_str(echoed).unwrap();
    let args: Vec<&str> = value["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();

    let sep = args
        .iter()
        .position(|a| *a == "--")
        .expect("-- before prompt");
    assert_eq!(
        &args[sep + 1..],
        &[prompt.as_str()],
        "prompt is one argv entry"
    );

    let flags = &args[..sep];
    assert_eq!(flags[0], "-p", "print mode first");
    for (flag, value) in [
        ("--output-format", "stream-json"),
        ("--permission-mode", "acceptEdits"),
        ("--permission-prompts", "none"),
    ] {
        let i = flags
            .iter()
            .position(|a| *a == flag)
            .unwrap_or_else(|| panic!("{flag} missing"));
        assert_eq!(flags[i + 1], value, "{flag}");
    }
    assert!(flags.contains(&"--verbose"), "stream-json needs --verbose");
    for banned in [
        "--dangerously-skip-permissions",
        "--allow-dangerously-skip-permissions",
        "bypassPermissions",
    ] {
        assert!(!flags.contains(&banned), "{banned} is banned");
    }
}

#[tokio::test]
async fn kill_cancels_the_run_and_still_finishes_once() {
    let mut run = fake_engine()
        .run(&task("hang"), ctx("r-kill"))
        .expect("spawn fake claude");
    let mut events = run.take_events();
    assert_eq!(
        events.next().await,
        Some(RunEvent::Started {
            run_id: "r-kill".into()
        })
    );
    // Wait for the init line so we know the child is up.
    let init = events.next().await;
    assert!(matches!(init, Some(RunEvent::Output { .. })), "{init:?}");
    run.kill();
    let rest: Vec<RunEvent> = tokio::time::timeout(Duration::from_secs(8), events.collect())
        .await
        .expect("kill hung");
    assert_eq!(run.wait().await, ExitReason::Cancelled);
    assert_eq!(
        rest.last(),
        Some(&RunEvent::Finished {
            run_id: "r-kill".into(),
            ok: false,
        })
    );
    assert_eq!(count(&rest, |e| matches!(e, RunEvent::Finished { .. })), 1);
}

#[tokio::test]
async fn detect_without_binary_reports_not_installed_not_error() {
    let missing = std::env::temp_dir().join("idle-app-no-such-claude-binary");
    let info = ClaudeEngine::with_program(&missing)
        .detect()
        .await
        .expect("missing binary is a state, not an error");
    assert_eq!(
        info,
        DetectInfo {
            installed: false,
            version: None,
            signed_in: false,
        }
    );
}

/// Points the fake's `auth status` at a fixture; restores the env on drop.
/// Tests using it are `#[serial]` because set_var races other env reads.
struct AuthFixture(Option<std::ffi::OsString>);

impl AuthFixture {
    fn set(name: &str) -> Self {
        let previous = std::env::var_os("IDLE_FAKE_CLI_AUTH_FIXTURE");
        std::env::set_var("IDLE_FAKE_CLI_AUTH_FIXTURE", fixture(name));
        Self(previous)
    }
}

impl Drop for AuthFixture {
    fn drop(&mut self) {
        match self.0.take() {
            Some(v) => std::env::set_var("IDLE_FAKE_CLI_AUTH_FIXTURE", v),
            None => std::env::remove_var("IDLE_FAKE_CLI_AUTH_FIXTURE"),
        }
    }
}

/// Also the guard for `raw_stdout_line`: the version below is plain text,
/// so it reaches the adapter as a Runner `Error` event and only comes back
/// as `9.8.7` if the adapter still understands the Runner's format.
#[tokio::test]
#[serial]
async fn detect_reports_version_and_signed_in_from_exit_code() {
    let _auth = AuthFixture::set("auth_status_signed_in.json");
    let info = fake_engine().detect().await.unwrap();
    assert_eq!(
        info,
        DetectInfo {
            installed: true,
            version: Some("9.8.7".into()),
            signed_in: true,
        }
    );
}

#[tokio::test]
#[serial]
async fn detect_reports_signed_out_from_exit_code() {
    let _auth = AuthFixture::set("auth_status_signed_out.json");
    let info = fake_engine().detect().await.unwrap();
    assert_eq!(
        info,
        DetectInfo {
            installed: true,
            version: Some("9.8.7".into()),
            signed_in: false,
        }
    );
}

#[tokio::test]
async fn install_and_login_refuse_until_pr19() {
    let engine = fake_engine();
    match engine.install().await {
        Err(EngineError::Install(msg)) => assert!(msg.contains("PR-19"), "{msg}"),
        other => panic!("install must refuse, got {other:?}"),
    }
    match engine.login().await {
        Err(EngineError::Login(msg)) => assert!(msg.contains("PR-19"), "{msg}"),
        other => panic!("login must refuse, got {other:?}"),
    }
}

#[test]
fn id_and_windows_follow_the_contract() {
    let engine = ClaudeEngine::new();
    assert_eq!(engine.id(), EngineId::Claude);
    assert_eq!(engine.windows(), default_windows(EngineId::Claude));
}

#[test]
fn registry_lists_claude_once() {
    let ids: Vec<EngineId> = idle_app_lib::engines::registry()
        .iter()
        .map(|e| e.id())
        .collect();
    assert_eq!(ids, vec![EngineId::Claude]);
}

/// Acceptance C. Runs one real task through the real binary and prints
/// every RunEvent. Needs `claude` installed and signed in; costs one turn.
#[tokio::test]
#[ignore = "needs a real claude binary and spends a real turn"]
async fn live_run_prints_every_event() {
    let dir = std::env::temp_dir().join("idle-app-claude-live");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut task = task(
        "Create a file named hello.txt in the current directory containing exactly the line: hello from idle-app. Then reply with the single word done.",
    );
    task.folder = dir.to_string_lossy().into_owned();
    let ctx = RunCtx {
        run_id: "live-1".into(),
        cwd: dir.clone(),
        timeout_secs: 300,
    };

    let mut run = ClaudeEngine::new().run(&task, ctx).expect("launch claude");
    let mut events = run.take_events();
    let mut seen = Vec::new();
    while let Some(event) = events.next().await {
        println!("{}", serde_json::to_string(&event).unwrap());
        seen.push(event);
    }
    let reason = run.wait().await;
    println!("exit reason: {reason:?}");

    assert_stream_shape(&seen, "live-1");
    assert_eq!(reason, ExitReason::Ok);
    assert_eq!(count(&seen, |e| matches!(e, RunEvent::Usage { .. })), 1);
    assert_eq!(
        std::fs::read_to_string(dir.join("hello.txt"))
            .unwrap()
            .trim(),
        "hello from idle-app"
    );
}
