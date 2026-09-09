use chrono::DateTime;
use idle_app_lib::contract::*;
use idle_app_lib::ipc::{AppState, METER_UPDATE, SCHEDULE_STATUS};
use idle_app_lib::scheduler::{decide, within_operating_hours, Idle, Snapshot};
use idle_app_lib::store::Store;
use serial_test::serial;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const NOW: &str = "2026-09-02T23:30:00Z";
const FAKE_CLI: &str = env!("CARGO_BIN_EXE_fake_cli");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(name)
}

fn task(id: &str, engine: EngineChoice) -> Task {
    Task {
        id: id.into(),
        prompt: format!("replay 0 {}", fixture("run_success.jsonl").display()),
        folder: env!("CARGO_MANIFEST_DIR").into(),
        size: TaskSize::S,
        engine,
        status: TaskStatus::Queued,
        created_at: NOW.into(),
        updated_at: NOW.into(),
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        tasks: vec![task("a", EngineChoice::Auto)],
        meters: Vec::new(),
        active: Vec::new(),
        cooldowns: Vec::new(),
        detect: vec![EngineStatus {
            engine: EngineId::Claude,
            detect: DetectInfo {
                installed: true,
                signed_in: true,
                version: None,
            },
        }],
        schedule: Schedule {
            enabled: true,
            ..Schedule::default()
        },
        now: DateTime::parse_from_rfc3339(NOW).unwrap(),
        idle: Idle::Minutes(10),
    }
}

#[test]
fn r1_quiet_hours_wrap_boundaries_all_day_and_off() {
    let mut s = snapshot();
    for (minute, expected) in [
        (23 * 60 + 30, true),
        (3 * 60, true),
        (22 * 60 + 59, false),
        (7 * 60, false),
        (23 * 60, true),
    ] {
        assert_eq!(within_operating_hours(&s.schedule, minute), expected);
    }
    s.schedule.quiet_end = s.schedule.quiet_start.clone();
    for minute in 0..1440 {
        assert!(within_operating_hours(&s.schedule, minute));
    }
    s.schedule.enabled = false;
    assert!(!within_operating_hours(&s.schedule, 23 * 60));
    assert!(decide(&s).starts.is_empty());
    assert_eq!(decide(&s).statuses[0].state, SchedulerState::Off);
}

#[test]
fn r1_local_offset_and_dst_repeated_hour_use_wall_time() {
    let mut s = snapshot();
    for now in [
        "2026-11-01T01:30:00-04:00",
        "2026-11-01T01:30:00-05:00",
        "2026-09-09T23:30:00+09:00",
    ] {
        s.now = DateTime::parse_from_rfc3339(now).unwrap();
        assert_eq!(decide(&s).starts, ["a"]);
    }
}

#[test]
fn r1_daytime_blocks_in_decision() {
    let mut s = snapshot();
    s.now = DateTime::parse_from_rfc3339("2026-09-02T10:00:00Z").unwrap();
    let d = decide(&s);
    assert!(d.starts.is_empty());
    assert_eq!(d.statuses[0].state, SchedulerState::Waiting);
    assert_eq!(d.statuses[0].reason, SchedulerReason::QuietHours);
}

#[tokio::test]
async fn cooldown_query_filters_history_with_offsets_and_uses_index() {
    let store = Store::open_in_memory().unwrap();
    for (hit, reset) in [
        ("2026-09-01T00:00:00Z", None),
        (NOW, None),
        ("2026-09-01T00:00:00Z", Some("2026-09-02T20:00:00-05:00")),
    ] {
        store
            .apply_run_event(
                EngineId::Claude,
                RunEvent::LimitHit {
                    run_id: "r".into(),
                    window: None,
                    resets_at: reset.map(str::to_string),
                },
                hit.into(),
                Usage::default(),
            )
            .await
            .unwrap();
    }
    let rows = store.cooldowns(NOW.into()).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|(_, t)| t == "2026-09-02T20:00:00-05:00"));
    let plan: String = store.run(|c| Ok(c.query_row("EXPLAIN QUERY PLAN SELECT * FROM limit_hits WHERE julianday(COALESCE(resets_at, hit_at)) >= julianday(?1) - ?2 / 1440.0", rusqlite::params![NOW, COOLDOWN_MINUTES], |r| r.get(3))?)).await.unwrap();
    assert!(plan.contains("idx_limit_hits_time"), "{plan}");
}

#[test]
fn r2_idle_gate_and_unknown() {
    let mut s = snapshot();
    s.idle = Idle::Minutes(9);
    assert_eq!(decide(&s).statuses[0].reason, SchedulerReason::NotIdle);
    assert!(decide(&s).starts.is_empty());
    s.idle = Idle::Unknown;
    assert_eq!(decide(&s).starts, ["a"]);
}

#[test]
fn r3_all_windows_reserve_and_unknown() {
    let mut s = snapshot();
    s.meters = default_windows(EngineId::Claude)
        .iter()
        .zip([99.0, 38.0])
        .map(|(w, pct)| MeterState {
            remaining_pct: Some(pct),
            source: MeterSource::Vendor,
            ..idle_app_lib::meter::seed(EngineId::Claude, w.kind).unwrap()
        })
        .collect();
    assert_eq!(decide(&s).starts, ["a"]);
    for size in [TaskSize::M, TaskSize::L] {
        s.tasks[0].size = size;
        assert_eq!(decide(&s).statuses[0].reason, SchedulerReason::Reserve);
        assert!(decide(&s).starts.is_empty());
    }
    for meter in &mut s.meters {
        meter.source = MeterSource::None;
    }
    assert_eq!(decide(&s).starts, ["a"]);
}

#[test]
fn r4_concurrency_busy_and_global_cap() {
    let mut s = snapshot();
    s.active.push(EngineId::Claude);
    assert!(decide(&s).starts.is_empty());
    assert_eq!(decide(&s).statuses[0].reason, SchedulerReason::Busy);
    s.active.clear();
    s.tasks
        .push(task("b", EngineChoice::Fixed(EngineId::Codex)));
    s.detect.push(EngineStatus {
        engine: EngineId::Codex,
        detect: s.detect[0].detect.clone(),
    });
    s.schedule.max_concurrent = 1;
    assert_eq!(decide(&s).starts.len(), 1);
    assert_eq!(decide(&s).statuses[1].reason, SchedulerReason::Busy);
    s.schedule.max_concurrent = 2;
    assert_eq!(decide(&s).starts.len(), 2);
}

#[test]
fn r5_fifo_and_unavailable_wait() {
    let mut s = snapshot();
    let mut older = task("z", EngineChoice::Auto);
    older.created_at = "2026-09-01T00:00:00Z".into();
    s.tasks.push(older);
    assert_eq!(decide(&s).starts, ["z"]);
    for (installed, signed_in) in [(false, true), (true, false)] {
        s.detect[0].detect.installed = installed;
        s.detect[0].detect.signed_in = signed_in;
        assert!(decide(&s).starts.is_empty());
        assert_eq!(
            decide(&s).statuses[0].reason,
            SchedulerReason::EngineUnavailable
        );
    }
    s.detect.clear();
    assert_eq!(
        decide(&s).statuses[0].reason,
        SchedulerReason::EngineUnavailable
    );
    assert!(s.tasks.iter().all(|t| t.status == TaskStatus::Queued));
}

async fn fake_state() -> AppState {
    std::env::set_var(
        "IDLE_FAKE_CLI_AUTH_FIXTURE",
        fixture("auth_status_signed_in.json"),
    );
    let mut state = AppState::new(Store::open_in_memory().unwrap());
    state.claude_program = Some(FAKE_CLI.into());
    state.clock = Arc::new(|| DateTime::parse_from_rfc3339(NOW).unwrap());
    state.idle = Arc::new(|| Idle::Minutes(10));
    state.set_schedule(snapshot().schedule).await.unwrap();
    state
}

async fn wait_finished(store: &Store, id: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if store
                .list_runs(Some(id.into()))
                .await
                .unwrap()
                .iter()
                .any(|r| r.finished_at.is_some())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("run finishes");
}

#[tokio::test]
async fn r6_limit_hit_requeues_twice_then_fails_and_cooldown_survives_restart() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("scheduler-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    let store = Store::open(&path).unwrap();
    store.add_task(task("a", EngineChoice::Auto)).await.unwrap();
    for n in 1..=3 {
        let run = format!("r{n}");
        store
            .claim_task_and_insert_run("a".into(), run.clone(), NOW.into())
            .await
            .unwrap();
        store
            .finish_run(run, NOW.into(), ExitReason::LimitHit, Usage::default())
            .await
            .unwrap();
        assert_eq!(
            store.get_task("a".into()).await.unwrap().unwrap().status,
            if n == 3 {
                TaskStatus::Failed
            } else {
                TaskStatus::Queued
            }
        );
    }
    store
        .apply_run_event(
            EngineId::Claude,
            RunEvent::LimitHit {
                run_id: "r3".into(),
                window: None,
                resets_at: None,
            },
            NOW.into(),
            Usage::default(),
        )
        .await
        .unwrap();
    drop(store);
    let store = Store::open(&path).unwrap();
    let mut s = snapshot();
    s.cooldowns = store.cooldowns(NOW.into()).await.unwrap();
    let status = decide(&s).statuses.remove(0);
    assert_eq!(status.state, SchedulerState::Paused);
    assert_eq!(
        DateTime::parse_from_rfc3339(&status.until.unwrap()).unwrap(),
        s.now + chrono::Duration::minutes(60)
    );
    let reset = "2026-09-10T03:00:00Z";
    store
        .apply_run_event(
            EngineId::Claude,
            RunEvent::LimitHit {
                run_id: "r3".into(),
                window: None,
                resets_at: Some(reset.into()),
            },
            NOW.into(),
            Usage::default(),
        )
        .await
        .unwrap();
    s.cooldowns = store.cooldowns(NOW.into()).await.unwrap();
    assert_eq!(decide(&s).statuses[0].until.as_deref(), Some(reset));
    assert!(store
        .claim_task_and_insert_run("a".into(), "r4".into(), NOW.into())
        .await
        .unwrap_err()
        .to_string()
        .contains("cooldown"));
}

#[tokio::test]
async fn r7_restart_reconciles_unfinished_run_and_task() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("scheduler-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.db");
    let store = Store::open(&path).unwrap();
    store.add_task(task("a", EngineChoice::Auto)).await.unwrap();
    store
        .claim_task_and_insert_run("a".into(), "r".into(), NOW.into())
        .await
        .unwrap();
    assert!(Store::open(&path).is_err());
    assert!(store.list_runs(None).await.unwrap()[0]
        .finished_at
        .is_none());
    assert!(store.delete_task("a".into()).await.is_err());
    assert!(store.list_runs(None).await.unwrap()[0]
        .finished_at
        .is_none());
    drop(store);
    let store = Store::open_at(&path, NOW).unwrap();
    let run = store.list_runs(None).await.unwrap().remove(0);
    assert_eq!(run.exit_reason, Some(ExitReason::Failed));
    assert_eq!(run.finished_at.as_deref(), Some(NOW));
    let mut s = snapshot();
    s.tasks = store.list_tasks().await.unwrap();
    assert_eq!(s.tasks[0].status, TaskStatus::Failed);
    assert!(decide(&s).starts.is_empty());
}

#[tokio::test]
#[serial]
async fn r9_status_emits_once_per_change_and_detect_is_cached() {
    let state = fake_state().await;
    let events = Arc::new(Mutex::new(Vec::new()));
    for _ in 0..3 {
        let events = events.clone();
        state
            .scheduler_tick(move |name, payload| {
                if name == SCHEDULE_STATUS {
                    events.lock().unwrap().push(payload);
                }
            })
            .await
            .unwrap();
        std::env::set_var(
            "IDLE_FAKE_CLI_AUTH_FIXTURE",
            fixture("auth_status_signed_out.json"),
        );
    }
    assert_eq!(events.lock().unwrap().len(), 4);
    assert!(state.detect_engines().await.unwrap()[0].detect.signed_in);
    state.set_schedule(Schedule::default()).await.unwrap();
    let changed = events.clone();
    state
        .scheduler_tick(move |name, payload| {
            if name == SCHEDULE_STATUS {
                changed.lock().unwrap().push(payload);
            }
        })
        .await
        .unwrap();
    assert_eq!(events.lock().unwrap().len(), 8);
}

#[tokio::test]
#[serial]
async fn r10_fake_cli_runs_sequentially_then_limit_hit_pauses() {
    let state = fake_state().await;
    for id in ["a", "b"] {
        state
            .store
            .add_task(task(id, EngineChoice::Auto))
            .await
            .unwrap();
    }
    let meters = Arc::new(Mutex::new(Vec::new()));
    for id in ["a", "b"] {
        let meters = meters.clone();
        state
            .scheduler_tick(move |name, payload| {
                if name == METER_UPDATE {
                    meters.lock().unwrap().push(payload);
                }
            })
            .await
            .unwrap();
        wait_finished(&state.store, id).await;
        assert_eq!(
            state
                .store
                .get_task(id.into())
                .await
                .unwrap()
                .unwrap()
                .status,
            TaskStatus::Done
        );
        if id == "a" {
            assert_eq!(
                state
                    .store
                    .get_task("b".into())
                    .await
                    .unwrap()
                    .unwrap()
                    .status,
                TaskStatus::Queued
            );
        }
    }
    assert!(!meters.lock().unwrap().is_empty());
    let runs = state.store.list_runs(None).await.unwrap();
    assert!(runs[0].finished_at.as_ref().unwrap() <= &runs[1].started_at);
    let mut limited = task("c", EngineChoice::Auto);
    limited.prompt = format!(
        "replay 1 {}",
        fixture("run_limit_hit.synthetic.jsonl").display()
    );
    state.store.add_task(limited).await.unwrap();
    state.scheduler_tick(|_, _| {}).await.unwrap();
    wait_finished(&state.store, "c").await;
    assert_eq!(
        state
            .store
            .get_task("c".into())
            .await
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Queued
    );
    assert_eq!(
        state.store.list_runs(Some("c".into())).await.unwrap()[0].exit_reason,
        Some(ExitReason::LimitHit)
    );
    assert_eq!(
        state.schedule_status().await.unwrap()[0].state,
        SchedulerState::Paused
    );
}

#[tokio::test]
#[serial]
async fn r4_r8_click_is_busy_and_disabling_auto_preserves_run() {
    let state = fake_state().await;
    for id in ["a", "b"] {
        state
            .store
            .add_task(task(id, EngineChoice::Auto))
            .await
            .unwrap();
    }
    state
        .store
        .update_task(
            "a".into(),
            Some(format!(
                "delay 250 {}",
                task("a", EngineChoice::Auto).prompt
            )),
            None,
            None,
            None,
            None,
            NOW.into(),
        )
        .await
        .unwrap();
    state.run_now("a".into(), |_, _| {}).await.unwrap();
    assert_eq!(
        state.schedule_status().await.unwrap()[0].reason,
        SchedulerReason::Busy
    );
    assert!(state
        .run_next(EngineId::Claude, |_, _| {})
        .await
        .unwrap_err()
        .contains("already running"));
    state.set_schedule(Schedule::default()).await.unwrap();
    state.scheduler_tick(|_, _| {}).await.unwrap();
    wait_finished(&state.store, "a").await;
    assert_eq!(
        state
            .store
            .get_task("a".into())
            .await
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Done
    );
    assert_eq!(
        state
            .store
            .get_task("b".into())
            .await
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Queued
    );
    assert_eq!(state.store.list_runs(None).await.unwrap().len(), 1);
}

#[tokio::test]
async fn schedule_validation_defaults_and_persistence() {
    let store = Store::open_in_memory().unwrap();
    assert_eq!(store.get_schedule().await.unwrap(), Schedule::default());
    for value in ["1:00", "24:00", "12:60", "12-00", "aa:00", "😀:"] {
        let schedule = Schedule {
            quiet_start: value.into(),
            ..Schedule::default()
        };
        assert!(store.set_schedule(schedule).await.is_err());
    }
    for schedule in [
        Schedule {
            reserve_pct: 96,
            ..Schedule::default()
        },
        Schedule {
            idle_minutes: 121,
            ..Schedule::default()
        },
        Schedule {
            max_concurrent: 0,
            ..Schedule::default()
        },
        Schedule {
            max_concurrent: 5,
            ..Schedule::default()
        },
    ] {
        assert!(store.set_schedule(schedule).await.is_err());
    }
    let valid = Schedule {
        enabled: true,
        reserve_pct: 95,
        idle_minutes: 120,
        max_concurrent: 4,
        ..Schedule::default()
    };
    store.set_schedule(valid.clone()).await.unwrap();
    assert_eq!(store.get_schedule().await.unwrap(), valid);
}

#[tokio::test]
async fn schema_v4_repairs_each_partial_step_and_preserves_schedule() {
    for partial in [
        "",
        "CREATE TABLE schedule (id INTEGER PRIMARY KEY CHECK (id = 1));",
        "CREATE TABLE schedule (id INTEGER PRIMARY KEY CHECK (id = 1), config TEXT);",
    ] {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("migration-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(idle_app_lib::store::SCHEMA).unwrap();
        conn.execute_batch("DROP TABLE schedule; UPDATE schema_version SET version = 3;")
            .unwrap();
        conn.execute_batch(partial).unwrap();
        drop(conn);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.get_schedule().await.unwrap(), Schedule::default());
        store
            .set_schedule(snapshot().schedule.clone())
            .await
            .unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        assert_eq!(store.get_schedule().await.unwrap(), snapshot().schedule);
        let version = store
            .run(|c| {
                Ok(c.query_row("SELECT version FROM schema_version", [], |r| {
                    r.get::<_, i64>(0)
                })?)
            })
            .await
            .unwrap();
        assert_eq!(version, 4);
    }
}

#[tokio::test]
#[serial]
async fn run_next_ignores_auto_gates_but_honors_cooldown() {
    let mut state = fake_state().await;
    state.set_schedule(Schedule::default()).await.unwrap();
    state.clock = Arc::new(|| DateTime::parse_from_rfc3339("2026-09-02T12:00:00Z").unwrap());
    state.idle = Arc::new(|| Idle::Minutes(0));
    state
        .store
        .add_task(task("a", EngineChoice::Auto))
        .await
        .unwrap();
    state
        .store
        .apply_run_event(
            EngineId::Claude,
            RunEvent::WindowReading {
                run_id: "old".into(),
                window: LimitWindowKind::Weekly,
                utilization: 1.0,
                resets_at: None,
            },
            NOW.into(),
            Usage::default(),
        )
        .await
        .unwrap();
    state.run_next(EngineId::Claude, |_, _| {}).await.unwrap();
    wait_finished(&state.store, "a").await;
    state
        .store
        .add_task(task("b", EngineChoice::Auto))
        .await
        .unwrap();
    state
        .store
        .apply_run_event(
            EngineId::Claude,
            RunEvent::LimitHit {
                run_id: "old".into(),
                window: None,
                resets_at: None,
            },
            NOW.into(),
            Usage::default(),
        )
        .await
        .unwrap();
    assert!(state
        .run_next(EngineId::Claude, |_, _| {})
        .await
        .unwrap_err()
        .contains("cooldown"));
    assert!(state
        .run_now("b".into(), |_, _| {})
        .await
        .unwrap_err()
        .contains("cooldown"));
    assert_eq!(
        state
            .store
            .get_task("b".into())
            .await
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Queued
    );
}

#[tokio::test]
#[serial]
async fn scheduler_failures_are_not_retried() {
    let state = fake_state().await;
    let mut missing = task("a", EngineChoice::Auto);
    missing.folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(uuid::Uuid::new_v4().to_string())
        .to_string_lossy()
        .into_owned();
    state.store.add_task(missing).await.unwrap();
    state.scheduler_tick(|_, _| {}).await.unwrap();
    assert_eq!(
        state
            .store
            .get_task("a".into())
            .await
            .unwrap()
            .unwrap()
            .status,
        TaskStatus::Failed
    );
    state.scheduler_tick(|_, _| {}).await.unwrap();
    assert_eq!(state.store.list_runs(None).await.unwrap().len(), 1);
}

#[tokio::test]
#[serial]
async fn r6_click_limit_hits_requeue_twice_then_fail() {
    let mut state = fake_state().await;
    state.clock = Arc::new(|| DateTime::parse_from_rfc3339("2026-09-04T23:30:00Z").unwrap());
    let mut limited = task("a", EngineChoice::Auto);
    limited.prompt = format!(
        "replay 1 {}",
        fixture("run_limit_hit.synthetic.jsonl").display()
    );
    state.store.add_task(limited).await.unwrap();
    for n in 1..=3 {
        state.run_now("a".into(), |_, _| {}).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let runs = state.store.list_runs(None).await.unwrap();
                if runs.len() == n && runs.iter().all(|r| r.finished_at.is_some()) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            state
                .store
                .get_task("a".into())
                .await
                .unwrap()
                .unwrap()
                .status,
            if n == 3 {
                TaskStatus::Failed
            } else {
                TaskStatus::Queued
            }
        );
    }
}

#[tokio::test]
async fn terminal_outcomes_do_not_retry() {
    let store = Store::open_in_memory().unwrap();
    for (id, reason, expected) in [
        ("fail", ExitReason::Failed, TaskStatus::Failed),
        ("timeout", ExitReason::Timeout, TaskStatus::Failed),
        ("cancel", ExitReason::Cancelled, TaskStatus::Discarded),
    ] {
        store.add_task(task(id, EngineChoice::Auto)).await.unwrap();
        store
            .claim_task_and_insert_run(id.into(), id.into(), NOW.into())
            .await
            .unwrap();
        store
            .finish_run(id.into(), NOW.into(), reason, Usage::default())
            .await
            .unwrap();
        assert_eq!(
            store.get_task(id.into()).await.unwrap().unwrap().status,
            expected
        );
    }
}

#[tokio::test]
async fn r4_atomic_claim_guards_distinct_tasks_and_global_limit() {
    let store = Store::open_in_memory().unwrap();
    store
        .set_schedule(Schedule {
            max_concurrent: 1,
            ..Schedule::default()
        })
        .await
        .unwrap();
    for id in ["a", "b"] {
        store.add_task(task(id, EngineChoice::Auto)).await.unwrap();
    }
    let (a, b) = tokio::join!(
        store.claim_task_and_insert_run("a".into(), "ra".into(), NOW.into()),
        store.claim_task_and_insert_run("b".into(), "rb".into(), NOW.into())
    );
    assert_ne!(a.is_ok(), b.is_ok());
    store
        .add_task(task("c", EngineChoice::Fixed(EngineId::Codex)))
        .await
        .unwrap();
    assert!(store
        .claim_task_and_insert_run("c".into(), "rc".into(), NOW.into())
        .await
        .is_err());
    assert_eq!(store.list_runs(None).await.unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "needs the official CLI signed in and spends two turns"]
async fn live_scheduler_two_tasks() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("scheduler-live-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let state = AppState::new(Store::open_in_memory().unwrap());
    let detect = state.detect_engines().await.unwrap();
    assert!(detect
        .iter()
        .any(|d| d.engine == EngineId::Claude && d.detect.installed && d.detect.signed_in));
    println!("detect {}", serde_json::to_string(&detect).unwrap());
    let schedule = Schedule {
        enabled: true,
        quiet_start: "00:00".into(),
        quiet_end: "00:00".into(),
        idle_minutes: 0,
        ..Schedule::default()
    };
    state.set_schedule(schedule).await.unwrap();
    for id in ["a", "b"] {
        let mut t = task(id, EngineChoice::Auto);
        t.folder = dir.to_string_lossy().into_owned();
        t.prompt = format!("Write {id}.txt in the current directory with exactly scheduler ok. Do not run shell commands or read other files. Reply done.");
        state.store.add_task(t).await.unwrap();
    }
    loop {
        state
            .scheduler_tick(|name, payload| {
                if name == SCHEDULE_STATUS || name == METER_UPDATE {
                    println!("{name} {payload}");
                }
            })
            .await
            .unwrap();
        let tasks = state.store.list_tasks().await.unwrap();
        if tasks.iter().all(|t| {
            matches!(
                t.status,
                TaskStatus::Done | TaskStatus::Failed | TaskStatus::Discarded
            )
        }) {
            break;
        }
        let statuses = state.schedule_status().await.unwrap();
        if statuses[0].state == SchedulerState::Paused
            || statuses[0].reason == SchedulerReason::Reserve
        {
            panic!("live scheduler blocked: {:?}", statuses[0]);
        }
        tokio::time::sleep(Duration::from_secs(TICK_SECS)).await;
    }
    let runs = state.store.list_runs(None).await.unwrap();
    for run in &runs {
        println!(
            "run {} {} {:?} {:?}",
            run.task_id, run.started_at, run.finished_at, run.exit_reason
        );
    }
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.exit_reason == Some(ExitReason::Ok)));
    assert!(runs[0].finished_at.as_ref().unwrap() <= &runs[1].started_at);
    for id in ["a", "b"] {
        assert_eq!(
            std::fs::read_to_string(dir.join(format!("{id}.txt")))
                .unwrap()
                .trim(),
            "scheduler ok"
        );
    }
}
