use crate::contract::{
    default_windows, EngineChoice, EngineId, ExitReason, LimitWindowKind, MeterState, Run, Task,
    TaskSize, TaskStatus, Usage,
};
use rusqlite::{params, Connection, Row};
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub const SCHEMA: &str = include_str!("schema.sql");

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("lock: {0}")]
    Lock(String),
    #[error("join: {0}")]
    Join(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid: {0}")]
    Invalid(String),
}

impl From<StoreError> for rusqlite::Error {
    fn from(e: StoreError) -> Self {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    }
}

pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    crate::engines::claude::rfc3339_from_unix(secs)
}

pub fn size_to_str(s: TaskSize) -> &'static str {
    match s {
        TaskSize::S => "s",
        TaskSize::M => "m",
        TaskSize::L => "l",
    }
}

pub fn str_to_size(s: &str) -> Result<TaskSize, StoreError> {
    match s {
        "s" => Ok(TaskSize::S),
        "m" => Ok(TaskSize::M),
        "l" => Ok(TaskSize::L),
        _ => Err(StoreError::Invalid(format!("size {s}"))),
    }
}

pub fn status_to_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Queued => "queued",
        TaskStatus::Running => "running",
        TaskStatus::Done => "done",
        TaskStatus::Failed => "failed",
        TaskStatus::Discarded => "discarded",
    }
}

pub fn str_to_status(s: &str) -> Result<TaskStatus, StoreError> {
    match s {
        "queued" => Ok(TaskStatus::Queued),
        "running" => Ok(TaskStatus::Running),
        "done" => Ok(TaskStatus::Done),
        "failed" => Ok(TaskStatus::Failed),
        "discarded" => Ok(TaskStatus::Discarded),
        _ => Err(StoreError::Invalid(format!("status {s}"))),
    }
}

pub fn engine_id_to_str(e: EngineId) -> &'static str {
    match e {
        EngineId::Claude => "claude",
        EngineId::Codex => "codex",
        EngineId::Antigravity => "antigravity",
        EngineId::Grok => "grok",
    }
}

pub fn str_to_engine_id(s: &str) -> Result<EngineId, StoreError> {
    match s {
        "claude" => Ok(EngineId::Claude),
        "codex" => Ok(EngineId::Codex),
        "antigravity" => Ok(EngineId::Antigravity),
        "grok" => Ok(EngineId::Grok),
        _ => Err(StoreError::Invalid(format!("engine {s}"))),
    }
}

pub fn window_to_str(w: LimitWindowKind) -> &'static str {
    match w {
        LimitWindowKind::FiveHour => "fiveHour",
        LimitWindowKind::Daily => "daily",
        LimitWindowKind::Weekly => "weekly",
    }
}

pub fn str_to_window(s: &str) -> Result<LimitWindowKind, StoreError> {
    match s {
        "fiveHour" => Ok(LimitWindowKind::FiveHour),
        "daily" => Ok(LimitWindowKind::Daily),
        "weekly" => Ok(LimitWindowKind::Weekly),
        _ => Err(StoreError::Invalid(format!("window {s}"))),
    }
}

pub fn reason_to_str(r: ExitReason) -> &'static str {
    match r {
        ExitReason::Ok => "ok",
        ExitReason::Failed => "failed",
        ExitReason::LimitHit => "limitHit",
        ExitReason::Cancelled => "cancelled",
        ExitReason::Timeout => "timeout",
    }
}

pub fn str_to_reason(s: &str) -> Result<ExitReason, StoreError> {
    match s {
        "ok" => Ok(ExitReason::Ok),
        "failed" => Ok(ExitReason::Failed),
        "limitHit" => Ok(ExitReason::LimitHit),
        "cancelled" => Ok(ExitReason::Cancelled),
        "timeout" => Ok(ExitReason::Timeout),
        _ => Err(StoreError::Invalid(format!("reason {s}"))),
    }
}

fn row_to_task(row: &Row) -> rusqlite::Result<Task> {
    let size_str: String = row.get(3)?;
    let engine_str: String = row.get(4)?;
    let status_str: String = row.get(5)?;
    let engine = serde_json::from_str(&engine_str).map_err(StoreError::Json)?;
    Ok(Task {
        id: row.get(0)?,
        prompt: row.get(1)?,
        folder: row.get(2)?,
        size: str_to_size(&size_str)?,
        engine,
        status: str_to_status(&status_str)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn row_to_run(row: &Row) -> rusqlite::Result<Run> {
    let engine_str: String = row.get(2)?;
    let reason_str: Option<String> = row.get(5)?;
    let exit_reason = reason_str.as_deref().map(str_to_reason).transpose()?;
    Ok(Run {
        id: row.get(0)?,
        task_id: row.get(1)?,
        engine: str_to_engine_id(&engine_str)?,
        started_at: row.get(3)?,
        finished_at: row.get(4)?,
        exit_reason,
        usage: Usage {
            input: row.get::<_, i64>(6)? as u64,
            output: row.get::<_, i64>(7)? as u64,
            cache: row.get::<_, i64>(8)? as u64,
        },
        snapshot_id: row.get(9)?,
    })
}

fn row_to_meter(row: &Row) -> rusqlite::Result<MeterState> {
    let engine_str: String = row.get(0)?;
    let window_str: String = row.get(1)?;
    let capacity_est: Option<i64> = row.get(5)?;
    let calibrated: i64 = row.get(6)?;
    Ok(MeterState {
        engine: str_to_engine_id(&engine_str)?,
        window: str_to_window(&window_str)?,
        used: Usage {
            input: row.get::<_, i64>(2)? as u64,
            output: row.get::<_, i64>(3)? as u64,
            cache: row.get::<_, i64>(4)? as u64,
        },
        capacity_est: capacity_est.map(|v| v as u64),
        calibrated: calibrated != 0,
        remaining_pct: row.get(7)?,
        resets_at: row.get(8)?,
    })
}

const SELECT_TASK: &str =
    "SELECT id, prompt, folder, size, engine, status, created_at, updated_at FROM tasks";
const SELECT_RUN: &str =
    "SELECT id, task_id, engine, started_at, finished_at, exit_reason, used_input, used_output, used_cache, snapshot_id FROM runs";
const SELECT_METER: &str =
    "SELECT engine, window, used_input, used_output, used_cache, capacity_est, calibrated, remaining_pct, resets_at FROM meter_state";

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;",
        )?;
        conn.execute_batch(SCHEMA)?;
        for engine in [
            EngineId::Claude,
            EngineId::Codex,
            EngineId::Antigravity,
            EngineId::Grok,
        ] {
            for window in default_windows(engine) {
                conn.execute(
                    "INSERT OR IGNORE INTO meter_state (engine, window, used_input, used_output, used_cache, capacity_est, calibrated, remaining_pct, resets_at) VALUES (?1, ?2, 0, 0, 0, NULL, 0, NULL, NULL)",
                    params![engine_id_to_str(engine), window_to_str(window.kind)],
                )?;
            }
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub async fn run<F, R>(&self, f: F) -> Result<R, StoreError>
    where
        F: FnOnce(&mut Connection) -> Result<R, StoreError> + Send + 'static,
        R: Send + 'static,
    {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = conn.lock().map_err(|e| StoreError::Lock(e.to_string()))?;
            f(&mut guard)
        })
        .await
        .map_err(|e| StoreError::Join(e.to_string()))?
    }

    pub async fn list_tasks(&self) -> Result<Vec<Task>, StoreError> {
        self.run(|conn| {
            let mut stmt = conn.prepare(&format!("{SELECT_TASK} ORDER BY created_at ASC"))?;
            let tasks = stmt
                .query_map([], row_to_task)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(tasks)
        })
        .await
    }

    pub async fn get_task(&self, id: String) -> Result<Option<Task>, StoreError> {
        self.run(move |conn| {
            let mut stmt = conn.prepare(&format!("{SELECT_TASK} WHERE id = ?1"))?;
            let mut rows = stmt.query_map(params![id], row_to_task)?;
            rows.next().transpose().map_err(Into::into)
        })
        .await
    }

    pub async fn add_task(&self, task: Task) -> Result<Task, StoreError> {
        let ret = task.clone();
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO tasks (id, prompt, folder, size, engine, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    task.id,
                    task.prompt,
                    task.folder,
                    size_to_str(task.size),
                    serde_json::to_string(&task.engine)?,
                    status_to_str(task.status),
                    task.created_at,
                    task.updated_at,
                ],
            )?;
            Ok(ret)
        })
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_task(
        &self,
        id: String,
        prompt: Option<String>,
        folder: Option<String>,
        size: Option<TaskSize>,
        engine: Option<EngineChoice>,
        status: Option<TaskStatus>,
        updated_at: String,
    ) -> Result<Task, StoreError> {
        let engine_str = engine
            .map(|e| serde_json::to_string(&e))
            .transpose()
            .map_err(StoreError::Json)?;
        self.run(move |conn| {
            let rows_affected = conn.execute(
                "UPDATE tasks SET prompt = COALESCE(?1, prompt), folder = COALESCE(?2, folder), size = COALESCE(?3, size), engine = COALESCE(?4, engine), status = COALESCE(?5, status), updated_at = ?6 WHERE id = ?7",
                params![
                    prompt,
                    folder,
                    size.map(size_to_str),
                    engine_str,
                    status.map(status_to_str),
                    updated_at,
                    id,
                ],
            )?;
            if rows_affected == 0 {
                return Err(StoreError::NotFound(format!("task {id}")));
            }
            let mut stmt = conn.prepare(&format!("{SELECT_TASK} WHERE id = ?1"))?;
            let mut rows = stmt.query_map(params![id], row_to_task)?;
            rows.next()
                .transpose()?
                .ok_or_else(|| StoreError::NotFound(format!("task {id}")))
        })
        .await
    }

    pub async fn delete_task(&self, id: String) -> Result<(), StoreError> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM runs WHERE task_id = ?1", params![id])?;
            tx.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
            tx.commit()?;
            Ok(())
        })
        .await
    }

    pub async fn claim_task_and_insert_run(
        &self,
        task_id: String,
        run_id: String,
        started_at: String,
    ) -> Result<(Task, Run), StoreError> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let mut stmt = tx.prepare(&format!("{SELECT_TASK} WHERE id = ?1"))?;
            let mut rows = stmt.query_map(params![task_id], row_to_task)?;
            let task = rows
                .next()
                .transpose()?
                .ok_or_else(|| StoreError::NotFound(format!("task {task_id}")))?;
            drop(rows);
            drop(stmt);

            if task.status == TaskStatus::Running {
                return Err(StoreError::Invalid(format!("task {task_id} is already running")));
            }

            let engine_id = match task.engine {
                EngineChoice::Fixed(id) => id,
                EngineChoice::Auto => EngineId::Claude,
            };

            let run = Run {
                id: run_id,
                task_id: task.id.clone(),
                engine: engine_id,
                started_at: started_at.clone(),
                finished_at: None,
                exit_reason: None,
                usage: Usage::default(),
                snapshot_id: None,
            };

            tx.execute(
                "UPDATE tasks SET status = 'running', updated_at = ?1 WHERE id = ?2",
                params![started_at, task.id],
            )?;

            tx.execute(
                "INSERT INTO runs (id, task_id, engine, started_at, finished_at, exit_reason, used_input, used_output, used_cache, snapshot_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    run.id,
                    run.task_id,
                    engine_id_to_str(run.engine),
                    run.started_at,
                    run.finished_at,
                    run.exit_reason.map(reason_to_str),
                    run.usage.input as i64,
                    run.usage.output as i64,
                    run.usage.cache as i64,
                    run.snapshot_id,
                ],
            )?;

            tx.commit()?;
            Ok((task, run))
        })
        .await
    }

    pub async fn insert_run(&self, run: Run) -> Result<(), StoreError> {
        self.run(move |conn| {
            conn.execute(
                "INSERT INTO runs (id, task_id, engine, started_at, finished_at, exit_reason, used_input, used_output, used_cache, snapshot_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    run.id,
                    run.task_id,
                    engine_id_to_str(run.engine),
                    run.started_at,
                    run.finished_at,
                    run.exit_reason.map(reason_to_str),
                    run.usage.input as i64,
                    run.usage.output as i64,
                    run.usage.cache as i64,
                    run.snapshot_id,
                ],
            )?;
            Ok(())
        })
        .await
    }

    pub async fn finish_run(
        &self,
        run_id: String,
        finished_at: String,
        exit_reason: ExitReason,
        usage: Usage,
    ) -> Result<(), StoreError> {
        self.run(move |conn| {
            let n = conn.execute(
                "UPDATE runs SET finished_at = ?1, exit_reason = ?2, used_input = ?3, used_output = ?4, used_cache = ?5 WHERE id = ?6",
                params![
                    finished_at,
                    reason_to_str(exit_reason),
                    usage.input as i64,
                    usage.output as i64,
                    usage.cache as i64,
                    run_id,
                ],
            )?;
            if n == 0 {
                return Err(StoreError::NotFound(format!("run {run_id}")));
            }
            Ok(())
        })
        .await
    }

    pub async fn list_runs(&self, task_id: Option<String>) -> Result<Vec<Run>, StoreError> {
        self.run(move |conn| {
            if let Some(ref tid) = task_id {
                let mut stmt = conn.prepare(&format!(
                    "{SELECT_RUN} WHERE task_id = ?1 ORDER BY started_at ASC"
                ))?;
                let rows: Result<Vec<_>, _> = stmt.query_map(params![tid], row_to_run)?.collect();
                Ok(rows?)
            } else {
                let mut stmt = conn.prepare(&format!("{SELECT_RUN} ORDER BY started_at ASC"))?;
                let rows: Result<Vec<_>, _> = stmt.query_map([], row_to_run)?.collect();
                Ok(rows?)
            }
        })
        .await
    }

    pub async fn get_meters(&self) -> Result<Vec<MeterState>, StoreError> {
        self.run(|conn| {
            let mut stmt = conn.prepare(SELECT_METER)?;
            let meters = stmt
                .query_map([], row_to_meter)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(meters)
        })
        .await
    }

    pub async fn update_meters_usage(
        &self,
        engine: EngineId,
        usage: Usage,
    ) -> Result<Vec<MeterState>, StoreError> {
        self.run(move |conn| {
            // Usage counts concurrently against all active limit windows (e.g. 5-hour and weekly) for this engine.
            conn.execute(
                "UPDATE meter_state SET used_input = used_input + ?1, used_output = used_output + ?2, used_cache = used_cache + ?3 WHERE engine = ?4",
                params![
                    usage.input as i64,
                    usage.output as i64,
                    usage.cache as i64,
                    engine_id_to_str(engine),
                ],
            )?;
            let mut stmt = conn.prepare(&format!("{SELECT_METER} WHERE engine = ?1"))?;
            let meters = stmt
                .query_map(params![engine_id_to_str(engine)], row_to_meter)?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(meters)
        })
        .await
    }

    pub async fn record_limit_hit(
        &self,
        engine: EngineId,
        window: LimitWindowKind,
        hit_at: String,
        resets_at: Option<String>,
        usage: Usage,
    ) -> Result<(), StoreError> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO limit_hits (engine, window, hit_at, resets_at, used_input, used_output, used_cache) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    engine_id_to_str(engine),
                    window_to_str(window),
                    hit_at,
                    resets_at,
                    usage.input as i64,
                    usage.output as i64,
                    usage.cache as i64,
                ],
            )?;
            tx.execute(
                "UPDATE meter_state SET resets_at = ?1, remaining_pct = 0.0 WHERE engine = ?2 AND window = ?3",
                params![
                    resets_at,
                    engine_id_to_str(engine),
                    window_to_str(window),
                ],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_executes_on_fresh_db() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(super::SCHEMA).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn schema_version_stays_single_row_on_reapply() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(super::SCHEMA).unwrap();
        conn.execute_batch(super::SCHEMA).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
        let id: i64 = conn
            .query_row("SELECT id FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, 1);
    }

    #[test]
    fn limit_hits_allows_duplicate_window_timestamps() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(super::SCHEMA).unwrap();
        let sql = "INSERT INTO limit_hits (engine, window, hit_at, used_input, used_output, used_cache) VALUES ('claude', 'fiveHour', 't', 1, 1, 1)";
        conn.execute(sql, []).unwrap();
        conn.execute(sql, []).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM limit_hits", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn tasks_crud_and_restart_survival() {
        let dir = std::env::temp_dir().join(format!("idle-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("test.db");

        let store = Store::open(&db_path).unwrap();
        let task = Task {
            id: "task-1".into(),
            prompt: "build app".into(),
            folder: dir.to_string_lossy().into_owned(),
            size: TaskSize::M,
            engine: EngineChoice::Auto,
            status: TaskStatus::Queued,
            created_at: "2026-09-04T00:00:00Z".into(),
            updated_at: "2026-09-04T00:00:00Z".into(),
        };
        store.add_task(task.clone()).await.unwrap();

        let updated = store
            .update_task(
                "task-1".into(),
                Some("build app now".into()),
                None,
                Some(TaskSize::L),
                None,
                Some(TaskStatus::Running),
                "2026-09-04T01:00:00Z".into(),
            )
            .await
            .unwrap();
        assert_eq!(updated.prompt, "build app now");
        assert_eq!(updated.size, TaskSize::L);
        assert_eq!(updated.status, TaskStatus::Running);

        drop(store);
        let store2 = Store::open(&db_path).unwrap();
        let loaded = store2
            .get_task("task-1".into())
            .await
            .unwrap()
            .expect("task survives");
        assert_eq!(loaded.prompt, "build app now");
        assert_eq!(loaded.size, TaskSize::L);

        let list = store2.list_tasks().await.unwrap();
        assert_eq!(list.len(), 1);

        store2
            .insert_run(Run {
                id: "r-task-1".into(),
                task_id: "task-1".into(),
                engine: EngineId::Claude,
                started_at: "2026-09-04T00:00:00Z".into(),
                finished_at: None,
                exit_reason: None,
                usage: Usage::default(),
                snapshot_id: None,
            })
            .await
            .unwrap();

        store2.delete_task("task-1".into()).await.unwrap();
        let empty = store2.list_tasks().await.unwrap();
        assert!(empty.is_empty());
        let empty_runs = store2.list_runs(Some("task-1".into())).await.unwrap();
        assert!(empty_runs.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn runs_and_meters_lifecycle() {
        let store = Store::open_in_memory().unwrap();
        let task = Task {
            id: "t1".into(),
            prompt: "p".into(),
            folder: "f".into(),
            size: TaskSize::S,
            engine: EngineChoice::Fixed(EngineId::Claude),
            status: TaskStatus::Queued,
            created_at: "t0".into(),
            updated_at: "t0".into(),
        };
        store.add_task(task).await.unwrap();

        let run = Run {
            id: "r1".into(),
            task_id: "t1".into(),
            engine: EngineId::Claude,
            started_at: "t0".into(),
            finished_at: None,
            exit_reason: None,
            usage: Usage::default(),
            snapshot_id: None,
        };
        store.insert_run(run).await.unwrap();

        store
            .finish_run(
                "r1".into(),
                "2026-09-04T01:00:00Z".into(),
                ExitReason::Ok,
                Usage {
                    input: 10,
                    output: 20,
                    cache: 30,
                },
            )
            .await
            .unwrap();

        let missing_err = store
            .finish_run(
                "nonexistent".into(),
                "2026-09-04T01:00:00Z".into(),
                ExitReason::Ok,
                Usage::default(),
            )
            .await;
        assert!(matches!(missing_err, Err(StoreError::NotFound(_))));

        let runs = store.list_runs(Some("t1".into())).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].exit_reason, Some(ExitReason::Ok));
        assert_eq!(runs[0].usage.input, 10);

        let meters = store.get_meters().await.unwrap();
        assert!(!meters.is_empty());

        store
            .update_meters_usage(
                EngineId::Claude,
                Usage {
                    input: 5,
                    output: 5,
                    cache: 5,
                },
            )
            .await
            .unwrap();
        let updated_meters = store.get_meters().await.unwrap();
        let claude_5h = updated_meters
            .iter()
            .find(|m| m.engine == EngineId::Claude && m.window == LimitWindowKind::FiveHour)
            .unwrap();
        assert_eq!(claude_5h.used.input, 5);

        store
            .record_limit_hit(
                EngineId::Claude,
                LimitWindowKind::FiveHour,
                "t2".into(),
                Some("2026-09-04T05:00:00Z".into()),
                Usage::default(),
            )
            .await
            .unwrap();
        let hit_meters = store.get_meters().await.unwrap();
        let hit_5h = hit_meters
            .iter()
            .find(|m| m.engine == EngineId::Claude && m.window == LimitWindowKind::FiveHour)
            .unwrap();
        assert_eq!(hit_5h.resets_at.as_deref(), Some("2026-09-04T05:00:00Z"));
        assert_eq!(hit_5h.remaining_pct, Some(0.0));
    }

    #[tokio::test]
    async fn foreign_keys_prevent_orphan_runs() {
        let store = Store::open_in_memory().unwrap();
        let run = Run {
            id: "r1".into(),
            task_id: "nonexistent".into(),
            engine: EngineId::Claude,
            started_at: "2026-09-04T00:00:00Z".into(),
            finished_at: None,
            exit_reason: None,
            usage: Usage::default(),
            snapshot_id: None,
        };
        assert!(store.insert_run(run).await.is_err());
    }

    #[tokio::test]
    async fn claim_task_and_insert_run_atomicity() {
        let store = Store::open_in_memory().unwrap();
        let task = Task {
            id: "t1".into(),
            prompt: "p".into(),
            folder: "f".into(),
            size: TaskSize::S,
            engine: EngineChoice::Auto,
            status: TaskStatus::Queued,
            created_at: "2026-09-04T00:00:00Z".into(),
            updated_at: "2026-09-04T00:00:00Z".into(),
        };
        store.add_task(task).await.unwrap();

        let (t, r) = store
            .claim_task_and_insert_run("t1".into(), "r1".into(), "2026-09-04T01:00:00Z".into())
            .await
            .unwrap();
        assert_eq!(t.id, "t1");
        assert_eq!(r.id, "r1");
        assert_eq!(r.engine, EngineId::Claude);

        let err = store
            .claim_task_and_insert_run("t1".into(), "r2".into(), "2026-09-04T01:00:00Z".into())
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::Invalid(_)));
    }
}
