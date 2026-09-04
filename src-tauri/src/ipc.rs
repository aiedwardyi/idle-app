use crate::contract::{
    EngineChoice, EngineId, EngineStatus, LimitWindowKind, MeterState, Run, RunEvent, Task,
    TaskSize, TaskStatus, Usage,
};
use crate::engines::claude::ClaudeEngine;
use crate::engines::{Engine, RunCtx};
use crate::store::{now_rfc3339, Store};
use futures::StreamExt;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

pub const LIST_TASKS: &str = "list_tasks";
pub const ADD_TASK: &str = "add_task";
pub const UPDATE_TASK: &str = "update_task";
pub const DELETE_TASK: &str = "delete_task";
pub const RUN_NOW: &str = "run_now";
pub const STOP_RUN: &str = "stop_run";
pub const LIST_RUNS: &str = "list_runs";
pub const GET_METERS: &str = "get_meters";
pub const GET_ENGINES: &str = "get_engines";

pub const RUN_EVENT: &str = "run_event";
pub const METER_UPDATE: &str = "meter_update";
pub const ENGINE_STATUS: &str = "engine_status";

pub struct AppState {
    pub store: Store,
    pub active_runs: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
    pub claude_program: Option<OsString>,
}

impl AppState {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            active_runs: Arc::new(Mutex::new(HashMap::new())),
            claude_program: None,
        }
    }

    pub async fn detect_engines(&self) -> Result<Vec<EngineStatus>, String> {
        let mut statuses = Vec::new();
        let engines: Vec<Box<dyn Engine>> = match &self.claude_program {
            Some(p) => vec![Box::new(ClaudeEngine::with_program(p))],
            None => crate::engines::registry(),
        };
        for engine in engines {
            let detect = engine.detect().await.map_err(|e| e.to_string())?;
            statuses.push(EngineStatus {
                engine: engine.id(),
                detect,
            });
        }
        Ok(statuses)
    }

    pub async fn stop_run(&self, run_id: &str) -> Result<(), String> {
        if let Ok(mut lock) = self.active_runs.lock() {
            if let Some(tx) = lock.remove(run_id) {
                let _ = tx.send(());
            }
        }
        Ok(())
    }

    pub async fn run_now<F>(&self, task_id: String, emit: F) -> Result<Run, String>
    where
        F: Fn(&str, serde_json::Value) + Send + Sync + 'static,
    {
        let task = self
            .store
            .get_task(task_id.clone())
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("task not found: {task_id}"))?;

        let engine_id = match task.engine {
            EngineChoice::Fixed(id) => id,
            EngineChoice::Auto => EngineId::Claude,
        };
        if engine_id != EngineId::Claude {
            return Err(format!("engine {engine_id:?} is not supported yet"));
        }

        let run_id = uuid::Uuid::new_v4().to_string();
        let started_at = now_rfc3339();
        let run = Run {
            id: run_id.clone(),
            task_id: task.id.clone(),
            engine: engine_id,
            started_at: started_at.clone(),
            finished_at: None,
            exit_reason: None,
            usage: Usage::default(),
            snapshot_id: None,
        };

        self.store
            .insert_run(run.clone())
            .await
            .map_err(|e| e.to_string())?;

        self.store
            .update_task(
                task.id.clone(),
                None,
                None,
                None,
                None,
                Some(TaskStatus::Running),
                started_at.clone(),
            )
            .await
            .map_err(|e| e.to_string())?;

        let engine = match &self.claude_program {
            Some(p) => ClaudeEngine::with_program(p),
            None => ClaudeEngine::new(),
        };
        let ctx = RunCtx {
            run_id: run_id.clone(),
            cwd: PathBuf::from(&task.folder),
            timeout_secs: 3600,
        };

        let mut engine_run = match engine.run(&task, ctx) {
            Ok(r) => r,
            Err(e) => {
                let err_msg = e.to_string();
                let finished_at = now_rfc3339();
                let _ = self
                    .store
                    .finish_run(
                        run_id,
                        finished_at.clone(),
                        crate::contract::ExitReason::Failed,
                        Usage::default(),
                    )
                    .await;
                let _ = self
                    .store
                    .update_task(
                        task.id,
                        None,
                        None,
                        None,
                        None,
                        Some(TaskStatus::Failed),
                        finished_at,
                    )
                    .await;
                return Err(err_msg);
            }
        };

        let (kill_tx, mut kill_rx) = oneshot::channel();
        self.active_runs
            .lock()
            .map_err(|e| e.to_string())?
            .insert(run_id.clone(), kill_tx);

        let store = self.store.clone();
        let active_runs = self.active_runs.clone();
        let run_id_bg = run_id.clone();
        let task_id_bg = task.id.clone();

        tokio::spawn(async move {
            let mut events = engine_run.take_events();
            let mut latest_usage = Usage::default();
            let mut limit_hit_resets = None;
            let mut killed = false;

            loop {
                tokio::select! {
                    _ = &mut kill_rx, if !killed => {
                        killed = true;
                        engine_run.kill();
                    }
                    event = events.next() => {
                        match event {
                            Some(ev) => {
                                match &ev {
                                    RunEvent::Usage { input, output, cache, .. } => {
                                        latest_usage = Usage { input: *input, output: *output, cache: *cache };
                                    }
                                    RunEvent::LimitHit { resets_at, .. } => {
                                        limit_hit_resets = resets_at.clone();
                                    }
                                    _ => {}
                                }
                                if let Ok(val) = serde_json::to_value(&ev) {
                                    emit(RUN_EVENT, val);
                                }
                            }
                            None => break,
                        }
                    }
                }
            }

            let reason = engine_run.wait().await;
            let finished_at = now_rfc3339();

            if let Some(resets) = limit_hit_resets {
                let _ = store
                    .record_limit_hit(
                        engine_id,
                        LimitWindowKind::FiveHour,
                        finished_at.clone(),
                        Some(resets),
                        latest_usage,
                    )
                    .await;
            }

            if latest_usage.input > 0 || latest_usage.output > 0 || latest_usage.cache > 0 {
                if let Ok(meters) = store.update_meters_usage(engine_id, latest_usage).await {
                    for m in meters {
                        if let Ok(val) = serde_json::to_value(&m) {
                            emit(METER_UPDATE, val);
                        }
                    }
                }
            }

            let _ = store
                .finish_run(run_id_bg.clone(), finished_at.clone(), reason, latest_usage)
                .await;

            let final_status = if reason == crate::contract::ExitReason::Ok {
                TaskStatus::Done
            } else {
                TaskStatus::Failed
            };
            let _ = store
                .update_task(
                    task_id_bg,
                    None,
                    None,
                    None,
                    None,
                    Some(final_status),
                    finished_at,
                )
                .await;

            if let Ok(mut lock) = active_runs.lock() {
                lock.remove(&run_id_bg);
            }
        });

        Ok(run)
    }
}

#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    state.store.list_tasks().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_task(
    prompt: String,
    folder: String,
    size: TaskSize,
    engine: EngineChoice,
    state: State<'_, AppState>,
) -> Result<Task, String> {
    let now = now_rfc3339();
    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        prompt,
        folder,
        size,
        engine,
        status: TaskStatus::Queued,
        created_at: now.clone(),
        updated_at: now,
    };
    state.store.add_task(task).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_task(
    id: String,
    prompt: Option<String>,
    folder: Option<String>,
    size: Option<TaskSize>,
    engine: Option<EngineChoice>,
    status: Option<TaskStatus>,
    state: State<'_, AppState>,
) -> Result<Task, String> {
    state
        .store
        .update_task(id, prompt, folder, size, engine, status, now_rfc3339())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_task(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.store.delete_task(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_now(
    task_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Run, String> {
    let app_handle = app.clone();
    state
        .run_now(task_id, move |name, payload| {
            let _ = app_handle.emit(name, payload);
        })
        .await
}

#[tauri::command]
pub async fn stop_run(run_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.stop_run(&run_id).await
}

#[tauri::command]
pub async fn list_runs(
    task_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Run>, String> {
    state
        .store
        .list_runs(task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_meters(state: State<'_, AppState>) -> Result<Vec<MeterState>, String> {
    state.store.get_meters().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_engines(state: State<'_, AppState>) -> Result<Vec<EngineStatus>, String> {
    state.detect_engines().await
}
