use idle_app_lib::contract::{EngineChoice, ExitReason, Task, TaskSize, TaskStatus};
use idle_app_lib::ipc::AppState;
use idle_app_lib::store::Store;
use std::path::PathBuf;
use std::time::Duration;

const FAKE_CLI: &str = env!("CARGO_BIN_EXE_fake_cli");

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/claude")
        .join(name)
}

#[tokio::test]
async fn run_now_streams_events_and_records_run() {
    let store = Store::open_in_memory().unwrap();
    let mut state = AppState::new(store.clone());
    state.claude_program = Some(FAKE_CLI.into());

    let prompt = format!("replay 0 {}", fixture("run_success.jsonl").display());
    let task = Task {
        id: "task-1".into(),
        prompt,
        folder: std::env::temp_dir().to_string_lossy().into_owned(),
        size: TaskSize::S,
        engine: EngineChoice::Auto,
        status: TaskStatus::Queued,
        created_at: "2026-09-04T00:00:00Z".into(),
        updated_at: "2026-09-04T00:00:00Z".into(),
    };
    store.add_task(task.clone()).await.unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let run = state
        .run_now(task.id.clone(), move |name, payload| {
            let _ = tx.try_send((name.to_string(), payload));
        })
        .await
        .unwrap();

    assert_eq!(run.task_id, task.id);

    let mut events = Vec::new();
    while let Some(item) = rx.recv().await {
        events.push(item);
        if events.len() >= 6 {
            break;
        }
    }
    assert!(!events.is_empty());

    tokio::time::sleep(Duration::from_millis(300)).await;

    let finished_task = store.get_task(task.id).await.unwrap().unwrap();
    assert_eq!(finished_task.status, TaskStatus::Done);

    let runs = store.list_runs(None).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].exit_reason, Some(ExitReason::Ok));
    assert!(runs[0].usage.input > 0);
}

#[tokio::test]
async fn stop_run_cancels_active_execution() {
    let store = Store::open_in_memory().unwrap();
    let mut state = AppState::new(store.clone());
    state.claude_program = Some(FAKE_CLI.into());

    let task = Task {
        id: "task-2".into(),
        prompt: "hang".into(),
        folder: std::env::temp_dir().to_string_lossy().into_owned(),
        size: TaskSize::S,
        engine: EngineChoice::Auto,
        status: TaskStatus::Queued,
        created_at: "2026-09-04T00:00:00Z".into(),
        updated_at: "2026-09-04T00:00:00Z".into(),
    };
    store.add_task(task.clone()).await.unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let run = state
        .run_now(task.id.clone(), move |name, payload| {
            let _ = tx.try_send((name.to_string(), payload));
        })
        .await
        .unwrap();

    let _ = rx.recv().await;

    state.stop_run(&run.id).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    let finished_task = store.get_task(task.id).await.unwrap().unwrap();
    assert_eq!(finished_task.status, TaskStatus::Failed);

    let runs = store.list_runs(None).await.unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].exit_reason, Some(ExitReason::Cancelled));
}
