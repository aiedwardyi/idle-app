use crate::contract::{
    EngineChoice, EngineId, EngineStatus, MeterState, Run, RunEvent, Schedule, SchedulerStatus,
    Task, TaskSize, TaskStatus, Usage, DETECT_CACHE_SECS, TICK_SECS,
};
use crate::engines::claude::ClaudeEngine;
use crate::engines::{Engine, RunCtx};
use crate::scheduler::{self, Idle, Snapshot};
use crate::store::Store;
use chrono::{DateTime, FixedOffset, SecondsFormat};
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

pub const GET_SCHEDULE: &str = "get_schedule";
pub const SET_SCHEDULE: &str = "set_schedule";
pub const GET_SCHEDULE_STATUS: &str = "get_schedule_status";
pub const RUN_NEXT: &str = "run_next";
pub const SCHEDULE_STATUS: &str = "schedule_status";

pub const RUN_EVENT: &str = "run_event";
pub const METER_UPDATE: &str = "meter_update";
pub const ENGINE_STATUS: &str = "engine_status";
pub const TASK_UPDATE: &str = "task_update";

/// A rejected claim is a race the next tick re-decides; anything else leaves the task stuck unless it is failed.
#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error("{0}")]
    Rejected(String),
    #[error("{0}")]
    Failed(String),
}

pub struct AppState {
    pub store: Store,
    pub active_runs: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
    pub claude_program: Option<OsString>,
    pub clock: Arc<dyn Fn() -> DateTime<FixedOffset> + Send + Sync>,
    pub idle: Arc<dyn Fn() -> Idle + Send + Sync>,
    admission: tokio::sync::Mutex<()>,
    detect_cache: tokio::sync::Mutex<Option<(std::time::Instant, Vec<EngineStatus>)>>,
    statuses: Mutex<Vec<SchedulerStatus>>,
}

impl AppState {
    async fn snapshot(&self, detect: Vec<EngineStatus>) -> Result<Snapshot, String> {
        let now = (self.clock)();
        let tasks = self.store.list_tasks().await.map_err(|e| e.to_string())?;
        let active = self
            .store
            .list_runs(None)
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|r| r.finished_at.is_none())
            .map(|r| r.engine)
            .collect();
        Ok(Snapshot {
            tasks,
            active,
            now,
            idle: (self.idle)(),
            meters: self
                .store
                .get_meters_at(now.to_utc().to_rfc3339_opts(SecondsFormat::Secs, true))
                .await
                .map_err(|e| e.to_string())?,
            detect,
            cooldowns: self
                .store
                .cooldowns(now.to_rfc3339())
                .await
                .map_err(|e| e.to_string())?,
            schedule: self.store.get_schedule().await.map_err(|e| e.to_string())?,
        })
    }

    pub async fn set_schedule(&self, schedule: Schedule) -> Result<(), String> {
        let _guard = self.admission.lock().await;
        self.store
            .set_schedule(schedule)
            .await
            .map_err(|e| e.to_string())
    }

    /// Probes spawn vendor CLIs, so skip them when `decide` cannot reach the detect gate anyway.
    async fn probe_detect(&self) -> Result<Vec<EngineStatus>, String> {
        if !self
            .store
            .get_schedule()
            .await
            .map_err(|e| e.to_string())?
            .enabled
        {
            return Ok(Vec::new());
        }
        let queued = self
            .store
            .list_tasks()
            .await
            .map_err(|e| e.to_string())?
            .iter()
            .any(|t| t.status == TaskStatus::Queued);
        if queued {
            self.detect_engines().await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn schedule_status(&self) -> Result<Vec<SchedulerStatus>, String> {
        Ok(scheduler::decide(&self.snapshot(self.probe_detect().await?).await?).statuses)
    }

    pub async fn run_next<F>(&self, engine: EngineId, emit: F) -> Result<Run, String>
    where
        F: Fn(&str, serde_json::Value) + Send + Sync + 'static,
    {
        let _guard = self.admission.lock().await;
        let task = self
            .store
            .list_tasks()
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|t| t.status == TaskStatus::Queued && scheduler::resolve(&t.engine) == engine)
            .min_by_key(|t| (t.created_at.clone(), t.id.clone()))
            .ok_or("no queued tasks")?;
        self.start_run(task.id, emit)
            .await
            .map_err(|e| e.to_string())
    }

    /// A run that reached `finish_run` already owns the task's status; only a start that died before that needs marking.
    async fn fail_task<F>(&self, task_id: &str, emit: &F) -> Result<(), String>
    where
        F: Fn(&str, serde_json::Value),
    {
        let Some(task) = self
            .store
            .get_task(task_id.to_string())
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(());
        };
        if !matches!(task.status, TaskStatus::Queued | TaskStatus::Running) {
            return Ok(());
        }
        let now = (self.clock)()
            .to_utc()
            .to_rfc3339_opts(SecondsFormat::Secs, true);
        let task = self
            .store
            .update_task(
                task.id,
                None,
                None,
                None,
                None,
                Some(TaskStatus::Failed),
                now,
            )
            .await
            .map_err(|e| e.to_string())?;
        emit(
            TASK_UPDATE,
            serde_json::to_value(&task).map_err(|e| e.to_string())?,
        );
        Ok(())
    }

    pub async fn scheduler_tick<F>(&self, emit: F) -> Result<(), String>
    where
        F: Fn(&str, serde_json::Value) + Clone + Send + Sync + 'static,
    {
        let detect = self.probe_detect().await?;
        let _guard = self.admission.lock().await;
        let now = (self.clock)()
            .to_utc()
            .to_rfc3339_opts(SecondsFormat::Secs, true);
        for meter in self
            .store
            .refresh_meters(now)
            .await
            .map_err(|e| e.to_string())?
        {
            emit(
                METER_UPDATE,
                serde_json::to_value(meter).map_err(|e| e.to_string())?,
            );
        }
        let decision = scheduler::decide(&self.snapshot(detect.clone()).await?);
        for task_id in decision.starts {
            match self.start_run(task_id.clone(), emit.clone()).await {
                Ok(_) => {}
                Err(StartError::Rejected(message)) => {
                    eprintln!("scheduler start {task_id}: {message}")
                }
                Err(StartError::Failed(message)) => {
                    eprintln!("scheduler start {task_id}: {message}");
                    if let Err(e) = self.fail_task(&task_id, &emit).await {
                        eprintln!("scheduler fail {task_id}: {e}");
                    }
                }
            }
        }
        let statuses = scheduler::decide(&self.snapshot(detect).await?).statuses;
        let mut previous = self.statuses.lock().map_err(|e| e.to_string())?;
        for status in &statuses {
            if !previous.contains(status) {
                emit(
                    SCHEDULE_STATUS,
                    serde_json::to_value(status).map_err(|e| e.to_string())?,
                );
            }
        }
        *previous = statuses;
        Ok(())
    }

    pub fn new(store: Store) -> Self {
        Self {
            store,
            active_runs: Arc::new(Mutex::new(HashMap::new())),
            claude_program: None,
            clock: Arc::new(scheduler::local_now),
            idle: Arc::new(scheduler::idle_probe),
            admission: tokio::sync::Mutex::new(()),
            detect_cache: tokio::sync::Mutex::new(None),
            statuses: Mutex::new(Vec::new()),
        }
    }

    pub async fn detect_engines(&self) -> Result<Vec<EngineStatus>, String> {
        let mut cache = self.detect_cache.lock().await;
        if let Some((at, statuses)) = &*cache {
            if at.elapsed().as_secs() < DETECT_CACHE_SECS {
                return Ok(statuses.clone());
            }
        }
        let mut statuses = Vec::new();
        let engines: Vec<Box<dyn Engine>> = match &self.claude_program {
            Some(p) => vec![Box::new(ClaudeEngine::with_program(p))],
            None => crate::engines::registry(),
        };
        for engine in engines {
            let detect = engine.detect().await.unwrap_or_else(|e| {
                eprintln!("detect: {e}");
                crate::contract::DetectInfo {
                    installed: false,
                    signed_in: false,
                    version: None,
                }
            });
            statuses.push(EngineStatus {
                engine: engine.id(),
                detect,
            });
        }
        *cache = Some((std::time::Instant::now(), statuses.clone()));
        Ok(statuses)
    }

    pub async fn stop_run(&self, run_id: &str) -> Result<(), String> {
        let mut lock = self
            .active_runs
            .lock()
            .map_err(|e| format!("active_runs lock poisoned: {e}"))?;
        if let Some(tx) = lock.remove(run_id) {
            let _ = tx.send(());
        }
        Ok(())
    }

    pub async fn run_now<F>(&self, task_id: String, emit: F) -> Result<Run, String>
    where
        F: Fn(&str, serde_json::Value) + Send + Sync + 'static,
    {
        let _guard = self.admission.lock().await;
        self.start_run(task_id, emit)
            .await
            .map_err(|e| e.to_string())
    }

    async fn start_run<F>(&self, task_id: String, emit: F) -> Result<Run, StartError>
    where
        F: Fn(&str, serde_json::Value) + Send + Sync + 'static,
    {
        let task = self
            .store
            .get_task(task_id.clone())
            .await
            .map_err(|e| StartError::Failed(e.to_string()))?
            .ok_or_else(|| StartError::Rejected(format!("task not found: {task_id}")))?;
        if scheduler::resolve(&task.engine) != EngineId::Claude {
            return Err(StartError::Failed("engine unavailable".into()));
        }
        let run_id = uuid::Uuid::new_v4().to_string();
        let started_at = (self.clock)()
            .to_utc()
            .to_rfc3339_opts(SecondsFormat::Secs, true);

        let (task, run) = self
            .store
            .claim_task_and_insert_run(task_id.clone(), run_id.clone(), started_at.clone())
            .await
            .map_err(|e| match e {
                crate::store::StoreError::ClaimRejected(msg) => StartError::Rejected(msg),
                crate::store::StoreError::NotFound(_) => {
                    StartError::Rejected(format!("task not found: {task_id}"))
                }
                other => StartError::Failed(other.to_string()),
            })?;

        if let Ok(val) = serde_json::to_value(&task) {
            emit(TASK_UPDATE, val);
        }

        let engine_id = run.engine;
        let engine = match &self.claude_program {
            Some(p) => ClaudeEngine::with_program(p),
            None => ClaudeEngine::new(),
        };
        let ctx = RunCtx {
            run_id: run_id.clone(),
            cwd: PathBuf::from(&task.folder),
            timeout_secs: task.size.timeout_secs(),
        };

        let mut engine_run = match engine.run(&task, ctx) {
            Ok(r) => r,
            Err(e) => {
                let err_msg = e.to_string();
                emit(
                    RUN_EVENT,
                    serde_json::to_value(RunEvent::Error {
                        run_id: run_id.clone(),
                        message: err_msg.clone(),
                    })
                    .map_err(|e| StartError::Failed(e.to_string()))?,
                );
                let finished_at = (self.clock)()
                    .to_utc()
                    .to_rfc3339_opts(SecondsFormat::Secs, true);
                match self
                    .store
                    .finish_run(
                        run_id,
                        finished_at,
                        crate::contract::ExitReason::Failed,
                        Usage::default(),
                    )
                    .await
                {
                    Ok(task) => {
                        if let Ok(val) = serde_json::to_value(&task) {
                            emit(TASK_UPDATE, val);
                        }
                    }
                    Err(e) => eprintln!("finish run: {e}"),
                }
                return Err(StartError::Failed(err_msg));
            }
        };

        let (kill_tx, mut kill_rx) = oneshot::channel();
        self.active_runs
            .lock()
            .map_err(|e| StartError::Failed(e.to_string()))?
            .insert(run_id.clone(), kill_tx);

        let store = self.store.clone();
        let active_runs = self.active_runs.clone();
        let run_id_bg = run_id.clone();
        let clock = self.clock.clone();

        tokio::spawn(async move {
            let mut events = engine_run.take_events();
            let mut latest_usage = Usage::default();
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
                                if let RunEvent::Usage { input, output, cache, .. } = &ev {
                                    latest_usage = Usage { input: *input, output: *output, cache: *cache };
                                }
                                if let Ok(val) = serde_json::to_value(&ev) {
                                    emit(RUN_EVENT, val);
                                }
                                if matches!(
                                    &ev,
                                    RunEvent::Started { .. }
                                        | RunEvent::Usage { .. }
                                        | RunEvent::WindowReading { .. }
                                        | RunEvent::LimitHit { .. }
                                ) {
                                    match store
                                        .apply_run_event(
                                            engine_id,
                                            ev,
                                            clock().to_utc().to_rfc3339_opts(SecondsFormat::Secs, true),
                                            latest_usage,
                                        )
                                        .await
                                    {
                                        Ok(changed) => {
                                            for m in changed {
                                                if let Ok(val) = serde_json::to_value(&m) {
                                                    emit(METER_UPDATE, val);
                                                }
                                            }
                                        }
                                        Err(e) => eprintln!("meter fold: {e}"),
                                    }
                                }
                            }
                            None => break,
                        }
                    }
                }
            }

            let reason = engine_run.wait().await;
            let finished_at = clock().to_utc().to_rfc3339_opts(SecondsFormat::Secs, true);

            match store
                .finish_run(run_id_bg.clone(), finished_at, reason, latest_usage)
                .await
            {
                Ok(task) => {
                    if let Ok(val) = serde_json::to_value(&task) {
                        emit(TASK_UPDATE, val);
                    }
                }
                Err(e) => eprintln!("finish run {run_id_bg}: {e}"),
            }

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
    let now = (state.clock)()
        .to_utc()
        .to_rfc3339_opts(SecondsFormat::Secs, true);
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
    let _guard = state.admission.lock().await;
    let now = (state.clock)()
        .to_utc()
        .to_rfc3339_opts(SecondsFormat::Secs, true);
    state
        .store
        .update_task(id, prompt, folder, size, engine, status, now)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_task(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let _guard = state.admission.lock().await;
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

#[tauri::command]
pub async fn get_schedule(state: State<'_, AppState>) -> Result<Schedule, String> {
    state.store.get_schedule().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_schedule(schedule: Schedule, state: State<'_, AppState>) -> Result<(), String> {
    state.set_schedule(schedule).await
}

#[tauri::command]
pub async fn get_schedule_status(
    state: State<'_, AppState>,
) -> Result<Vec<SchedulerStatus>, String> {
    state.schedule_status().await
}

#[tauri::command]
pub async fn run_next(
    engine: EngineId,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Run, String> {
    state
        .run_next(engine, move |name, payload| {
            let _ = app.emit(name, payload);
        })
        .await
}

pub fn spawn_scheduler(app: AppHandle) {
    use tauri::Manager;
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(TICK_SECS));
        // Skip missed ticks so waking from sleep never bursts queued starts.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let handle = app.clone();
            if let Err(e) = app
                .state::<AppState>()
                .scheduler_tick(move |name, payload| {
                    let _ = handle.emit(name, payload);
                })
                .await
            {
                eprintln!("scheduler: {e}");
            }
        }
    });
}
