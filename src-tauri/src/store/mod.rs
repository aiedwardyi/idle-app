use crate::contract::{
    default_windows, EngineChoice, EngineId, ExitReason, LimitWindowKind, MeterSource, MeterState,
    Run, RunEvent, Schedule, Task, TaskSize, TaskStatus, Usage, COOLDOWN_MINUTES, MAX_LIMIT_HITS,
};
use crate::meter::{apply, refresh, seed};
use rusqlite::{params, Connection, Row};
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub const SCHEMA: &str = include_str!("schema.sql");
pub const SCHEMA_VERSION: i64 = 4;

/// v1 -> v2: `limit_hits.window` becomes nullable. SQLite cannot drop a
/// NOT NULL in place, so the table is rebuilt; nothing references it, so
/// the drop is safe with foreign keys on.
const MIGRATE_V1_TO_V2: &str = "BEGIN;
CREATE TABLE limit_hits_v2 (
    id INTEGER PRIMARY KEY,
    engine TEXT NOT NULL,
    window TEXT,
    hit_at TEXT NOT NULL,
    resets_at TEXT,
    used_input INTEGER NOT NULL,
    used_output INTEGER NOT NULL,
    used_cache INTEGER NOT NULL
);
INSERT INTO limit_hits_v2 SELECT id, engine, window, hit_at, resets_at, used_input, used_output, used_cache FROM limit_hits;
DROP TABLE limit_hits;
ALTER TABLE limit_hits_v2 RENAME TO limit_hits;
CREATE INDEX IF NOT EXISTS idx_limit_hits_engine_window ON limit_hits (engine, window);
UPDATE schema_version SET version = 2 WHERE id = 1;
COMMIT;";

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
    /// A write that lost a race, not a broken store. Callers branch on this instead of matching message text.
    #[error("{0}")]
    ClaimRejected(String),
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

pub fn source_to_str(s: MeterSource) -> &'static str {
    match s {
        MeterSource::Vendor => "vendor",
        MeterSource::Estimate => "estimate",
        MeterSource::None => "none",
    }
}

pub fn str_to_source(s: &str) -> Result<MeterSource, StoreError> {
    match s {
        "vendor" => Ok(MeterSource::Vendor),
        "estimate" => Ok(MeterSource::Estimate),
        "none" => Ok(MeterSource::None),
        _ => Err(StoreError::Invalid(format!("source {s}"))),
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
        source: str_to_source(&row.get::<_, String>(9)?)?,
        observed_at: row.get(10)?,
    })
}

const SELECT_TASK: &str =
    "SELECT id, prompt, folder, size, engine, status, created_at, updated_at FROM tasks";
const SELECT_RUN: &str =
    "SELECT id, task_id, engine, started_at, finished_at, exit_reason, used_input, used_output, used_cache, snapshot_id FROM runs";
const SELECT_METER: &str =
    "SELECT engine, window, used_input, used_output, used_cache, capacity_est, calibrated, remaining_pct, resets_at, source, observed_at FROM meter_state";

/// Brings an older database up to [`SCHEMA_VERSION`]. A fresh database is
/// already current: `schema.sql` writes the version on its first insert.
fn migrate(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 =
        conn.query_row("SELECT version FROM schema_version", [], |row| row.get(0))?;
    if version < 2 {
        conn.execute_batch(MIGRATE_V1_TO_V2)?;
    }
    if version < 3 {
        if !table_has_column(conn, "meter_state", "source")? {
            conn.execute(
                "ALTER TABLE meter_state ADD COLUMN source TEXT NOT NULL DEFAULT 'none'",
                [],
            )?;
        }
        if !table_has_column(conn, "meter_state", "observed_at")? {
            conn.execute("ALTER TABLE meter_state ADD COLUMN observed_at TEXT", [])?;
        }
        conn.execute("UPDATE schema_version SET version = 3 WHERE id = 1", [])?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schedule (id INTEGER PRIMARY KEY CHECK (id = 1))",
    )?;
    if !table_has_column(conn, "schedule", "config")? {
        conn.execute("ALTER TABLE schedule ADD COLUMN config TEXT", [])?;
    }
    if version < 4 {
        conn.execute("UPDATE schema_version SET version = 4 WHERE id = 1", [])?;
    }
    conn.execute("CREATE INDEX IF NOT EXISTS idx_limit_hits_time ON limit_hits (julianday(COALESCE(resets_at, hit_at)))", [])?;
    Ok(())
}

/// A stored row that will not parse or validate falls back to the default, which has `enabled: false`.
/// Erroring here would brick every read; `set_schedule` is where a bad value is rejected loudly.
fn load_schedule(conn: &Connection) -> Result<Schedule, StoreError> {
    use rusqlite::OptionalExtension;
    static WARNED: std::sync::Once = std::sync::Once::new();
    let json: Option<String> = conn
        .query_row("SELECT config FROM schedule WHERE id = 1", [], |r| r.get(0))
        .optional()?
        .flatten();
    let Some(text) = json else {
        return Ok(Schedule::default());
    };
    Ok(serde_json::from_str::<Schedule>(&text)
        .ok()
        .filter(|s| s.validate().is_ok())
        .unwrap_or_else(|| {
            WARNED.call_once(|| eprintln!("stored schedule is unusable; using defaults"));
            Schedule::default()
        }))
}

fn load_cooldowns(conn: &Connection, now: &str) -> Result<Vec<(EngineId, String)>, StoreError> {
    let mut stmt = conn.prepare("SELECT engine, hit_at, resets_at FROM limit_hits WHERE julianday(COALESCE(resets_at, hit_at)) >= julianday(?1) - ?2 / 1440.0")?;
    let rows = stmt.query_map(params![now, COOLDOWN_MINUTES], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut cooldowns = Vec::new();
    for row in rows {
        let (engine, hit, reset) = row?;
        let until = reset.or_else(|| {
            chrono::DateTime::parse_from_rfc3339(&hit)
                .ok()
                .map(|t| (t + chrono::Duration::minutes(COOLDOWN_MINUTES)).to_rfc3339())
        });
        if let Some(until) = until {
            cooldowns.push((str_to_engine_id(&engine)?, until));
        }
    }
    Ok(cooldowns)
}

fn reconcile(conn: &mut Connection, now: &str) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE tasks SET status = 'failed', updated_at = ?1 WHERE id IN (SELECT task_id FROM runs WHERE finished_at IS NULL)", [now])?;
    tx.execute(
        "UPDATE runs SET exit_reason = 'failed', finished_at = ?1 WHERE finished_at IS NULL",
        [now],
    )?;
    tx.commit()?;
    Ok(())
}

/// Callers must pass hardcoded table-name literals only. PRAGMA takes no bind parameters.
fn table_has_column(
    conn: &Connection,
    table: &'static str,
    column: &'static str,
) -> Result<bool, StoreError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn seed_default_windows(conn: &Connection) -> Result<(), StoreError> {
    for engine in [
        EngineId::Claude,
        EngineId::Codex,
        EngineId::Antigravity,
        EngineId::Grok,
    ] {
        for window in default_windows(engine) {
            conn.execute(
                "INSERT OR IGNORE INTO meter_state (engine, window, used_input, used_output, used_cache, capacity_est, calibrated, remaining_pct, resets_at, source, observed_at) VALUES (?1, ?2, 0, 0, 0, NULL, 0, NULL, NULL, ?3, NULL)",
                params![
                    engine_id_to_str(engine),
                    window_to_str(window.kind),
                    source_to_str(MeterSource::None),
                ],
            )?;
        }
    }
    Ok(())
}

fn load_all_meters(conn: &Connection) -> Result<Vec<MeterState>, StoreError> {
    let mut stmt = conn.prepare(SELECT_METER)?;
    let meters = stmt
        .query_map([], row_to_meter)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(meters)
}

fn load_engine_meters(conn: &Connection, engine: EngineId) -> Result<Vec<MeterState>, StoreError> {
    let mut stmt = conn.prepare(&format!("{SELECT_METER} WHERE engine = ?1"))?;
    let meters = stmt
        .query_map(params![engine_id_to_str(engine)], row_to_meter)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(meters)
}

fn write_meter(conn: &Connection, m: &MeterState) -> Result<(), StoreError> {
    let n = conn.execute(
        "UPDATE meter_state SET used_input = ?1, used_output = ?2, used_cache = ?3, capacity_est = ?4, calibrated = ?5, remaining_pct = ?6, resets_at = ?7, source = ?8, observed_at = ?9 WHERE engine = ?10 AND window = ?11",
        params![
            m.used.input as i64,
            m.used.output as i64,
            m.used.cache as i64,
            m.capacity_est.map(|v| v as i64),
            if m.calibrated { 1 } else { 0 },
            m.remaining_pct,
            m.resets_at,
            source_to_str(m.source),
            m.observed_at,
            engine_id_to_str(m.engine),
            window_to_str(m.window),
        ],
    )?;
    if n == 0 {
        return Err(StoreError::NotFound(format!(
            "meter {} {}",
            engine_id_to_str(m.engine),
            window_to_str(m.window)
        )));
    }
    Ok(())
}

fn write_limit_hit(
    conn: &Connection,
    engine: EngineId,
    window: Option<LimitWindowKind>,
    hit_at: &str,
    resets_at: Option<String>,
    usage: Usage,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO limit_hits (engine, window, hit_at, resets_at, used_input, used_output, used_cache) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            engine_id_to_str(engine),
            window.map(window_to_str),
            hit_at,
            resets_at.clone(),
            usage.input as i64,
            usage.output as i64,
            usage.cache as i64,
        ],
    )?;
    if let Some(kind) = window {
        conn.execute(
            "UPDATE meter_state SET resets_at = COALESCE(?1, resets_at), remaining_pct = 0.0, source = ?2, observed_at = ?3 WHERE engine = ?4 AND window = ?5",
            params![
                resets_at,
                source_to_str(MeterSource::Vendor),
                hit_at,
                engine_id_to_str(engine),
                window_to_str(kind),
            ],
        )?;
    }
    Ok(())
}

struct SyncedMeters {
    all: Vec<MeterState>,
    changed: Vec<MeterState>,
}

fn sync_meters(conn: &Connection, now: &str) -> Result<SyncedMeters, StoreError> {
    seed_default_windows(conn)?;
    let loaded = load_all_meters(conn)?;
    let mut all = Vec::with_capacity(loaded.len());
    let mut changed = Vec::new();
    for row in loaded {
        let next = refresh(row.clone(), now);
        if next != row {
            write_meter(conn, &next)?;
            changed.push(next.clone());
        }
        all.push(next);
    }
    Ok(SyncedMeters { all, changed })
}

fn fold_run_event(
    conn: &Connection,
    engine: EngineId,
    event: &RunEvent,
    now: &str,
    run_usage: Usage,
) -> Result<Vec<MeterState>, StoreError> {
    if let RunEvent::WindowReading { window, .. } = event {
        if seed(engine, *window).is_none() {
            eprintln!(
                "warn: windowReading {window:?} for {engine:?} ignored, not in default_windows"
            );
        }
    }
    let before = load_engine_meters(conn, engine)?;
    let mut changed = Vec::new();
    // LimitHit still refresh-rolls other windows here; retain+reload below swaps in write_limit_hit's row.
    for row in &before {
        let next = apply(row.clone(), engine, event, now);
        if next != *row {
            write_meter(conn, &next)?;
            changed.push(next);
        }
    }
    if let RunEvent::LimitHit {
        window, resets_at, ..
    } = event
    {
        write_limit_hit(conn, engine, *window, now, resets_at.clone(), run_usage)?;
        if let Some(kind) = window {
            changed.retain(|m| m.window != *kind);
            if let Some(row) = load_engine_meters(conn, engine)?
                .into_iter()
                .find(|m| m.window == *kind)
            {
                let old = before.iter().find(|m| m.window == *kind);
                if old != Some(&row) {
                    changed.push(row);
                }
            }
        }
    }
    Ok(changed)
}

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
    owner: Option<Arc<std::fs::File>>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        Self::open_at(path, &now_rfc3339())
    }

    pub fn open_at(path: impl AsRef<Path>, now: &str) -> Result<Self, StoreError> {
        let path = path.as_ref();
        let owner = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
            .map_err(|e| StoreError::Lock(e.to_string()))?;
        owner
            .try_lock()
            .map_err(|e| StoreError::Lock(format!("database already in use: {e}")))?;
        let mut store = Self::init(Connection::open(path)?, now)?;
        store.owner = Some(Arc::new(owner));
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::init(Connection::open_in_memory()?, &now_rfc3339())
    }

    fn init(mut conn: Connection, now: &str) -> Result<Self, StoreError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;",
        )?;
        conn.execute_batch(SCHEMA)?;
        migrate(&conn)?;
        seed_default_windows(&conn)?;
        reconcile(&mut conn, now)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            owner: None,
        })
    }

    pub async fn run<F, R>(&self, f: F) -> Result<R, StoreError>
    where
        F: FnOnce(&mut Connection) -> Result<R, StoreError> + Send + 'static,
        R: Send + 'static,
    {
        let conn = self.conn.clone();
        let owner = self.owner.clone();
        tokio::task::spawn_blocking(move || {
            let _owner = owner;
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

    pub async fn get_schedule(&self) -> Result<Schedule, StoreError> {
        self.run(|conn| load_schedule(conn)).await
    }

    pub async fn set_schedule(&self, schedule: Schedule) -> Result<(), StoreError> {
        schedule.validate().map_err(StoreError::Invalid)?;
        self.run(move |conn| {
            conn.execute("INSERT INTO schedule (id, config) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET config = excluded.config", [serde_json::to_string(&schedule)?])?;
            Ok(())
        }).await
    }

    pub async fn cooldowns(&self, now: String) -> Result<Vec<(EngineId, String)>, StoreError> {
        self.run(move |conn| load_cooldowns(conn, &now)).await
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
            let busy: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1 AND finished_at IS NULL)",
                [&id],
                |r| r.get(0),
            )?;
            if busy {
                return Err(StoreError::Invalid("cannot delete a running task".into()));
            }
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

            // Only queued tasks are claimable, so the third limit hit cannot be silently re-run.
            if task.status != TaskStatus::Queued {
                return Err(StoreError::ClaimRejected(format!("task {task_id} is not queued")));
            }

            let engine_id = crate::scheduler::resolve(&task.engine);
            let active: i64 = tx.query_row("SELECT COUNT(*) FROM runs WHERE finished_at IS NULL", [], |r| r.get(0))?;
            let busy: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM runs WHERE engine = ?1 AND finished_at IS NULL)", [engine_id_to_str(engine_id)], |r| r.get(0))?;
            if busy {
                return Err(StoreError::ClaimRejected("engine already running".into()));
            }
            if active >= i64::from(load_schedule(&tx)?.max_concurrent) {
                return Err(StoreError::ClaimRejected("concurrency full".into()));
            }
            let now = chrono::DateTime::parse_from_rfc3339(&started_at).map_err(|e| StoreError::Invalid(e.to_string()))?;
            if load_cooldowns(&tx, &started_at)?.iter().any(|(engine, until)| *engine == engine_id && chrono::DateTime::parse_from_rfc3339(until).is_ok_and(|t| t > now)) {
                return Err(StoreError::ClaimRejected("engine cooldown".into()));
            }

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

            // Re-read so callers publish the claimed row, not the queued one they matched on.
            let mut stmt = tx.prepare(&format!("{SELECT_TASK} WHERE id = ?1"))?;
            let mut rows = stmt.query_map(params![run.task_id], row_to_task)?;
            let claimed = rows
                .next()
                .transpose()?
                .ok_or_else(|| StoreError::NotFound(format!("task {}", run.task_id)))?;
            drop(rows);
            drop(stmt);

            tx.commit()?;
            Ok((claimed, run))
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
    ) -> Result<Task, StoreError> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let n = tx.execute(
                "UPDATE runs SET finished_at = ?1, exit_reason = ?2, used_input = ?3, used_output = ?4, used_cache = ?5 WHERE id = ?6 AND finished_at IS NULL",
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
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM runs WHERE id = ?1)",
                    [&run_id],
                    |r| r.get(0),
                )?;
                return Err(if exists {
                    StoreError::ClaimRejected(format!("run {run_id} is already finished"))
                } else {
                    StoreError::NotFound(format!("run {run_id}"))
                });
            }
            let hits: usize = tx.query_row("SELECT COUNT(*) FROM runs WHERE task_id = (SELECT task_id FROM runs WHERE id = ?1) AND exit_reason = 'limitHit'", [&run_id], |r| r.get(0))?;
            let status = match exit_reason {
                ExitReason::Ok => TaskStatus::Done,
                ExitReason::Cancelled => TaskStatus::Discarded,
                ExitReason::LimitHit if hits < MAX_LIMIT_HITS => TaskStatus::Queued,
                _ => TaskStatus::Failed,
            };
            tx.execute("UPDATE tasks SET status = ?1, updated_at = ?2 WHERE id = (SELECT task_id FROM runs WHERE id = ?3)", params![status_to_str(status), finished_at, run_id])?;
            let mut stmt = tx.prepare(&format!("{SELECT_TASK} WHERE id = (SELECT task_id FROM runs WHERE id = ?1)"))?;
            let mut rows = stmt.query_map(params![run_id], row_to_task)?;
            let task = rows
                .next()
                .transpose()?
                .ok_or_else(|| StoreError::NotFound(format!("task for run {run_id}")))?;
            drop(rows);
            drop(stmt);
            tx.commit()?;
            Ok(task)
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
        self.get_meters_at(now_rfc3339()).await
    }

    pub async fn get_meters_at(&self, now: String) -> Result<Vec<MeterState>, StoreError> {
        self.run(move |conn| Ok(sync_meters(conn, &now)?.all)).await
    }

    pub async fn peek_meters_at(&self, now: String) -> Result<Vec<MeterState>, StoreError> {
        self.run(move |conn| {
            Ok(load_all_meters(conn)?
                .into_iter()
                .map(|row| refresh(row, &now))
                .collect())
        })
        .await
    }

    pub async fn refresh_meters(&self, now: String) -> Result<Vec<MeterState>, StoreError> {
        self.run(move |conn| Ok(sync_meters(conn, &now)?.changed))
            .await
    }

    pub async fn apply_run_event(
        &self,
        engine: EngineId,
        event: RunEvent,
        now: String,
        run_usage: Usage,
    ) -> Result<Vec<MeterState>, StoreError> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            let changed = fold_run_event(&tx, engine, &event, &now, run_usage)?;
            tx.commit()?;
            Ok(changed)
        })
        .await
    }

    #[cfg(test)]
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

    /// Records a limit hit. `window` is `None` when the engine could not tell
    /// which window was exhausted: the hit is still ground truth, so the row
    /// is written, but no meter moves. A guessed bucket is worse than a
    /// meter that stays where it was.
    #[cfg(test)]
    pub async fn record_limit_hit(
        &self,
        engine: EngineId,
        window: Option<LimitWindowKind>,
        hit_at: String,
        resets_at: Option<String>,
        usage: Usage,
    ) -> Result<(), StoreError> {
        self.run(move |conn| {
            let tx = conn.transaction()?;
            write_limit_hit(&tx, engine, window, &hit_at, resets_at, usage)?;
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
        assert_eq!(version, SCHEMA_VERSION);
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

        assert!(store2.delete_task("task-1".into()).await.is_err());
        store2
            .finish_run(
                "r-task-1".into(),
                "2026-09-04T01:00:00Z".into(),
                ExitReason::Ok,
                Usage::default(),
            )
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
        assert_eq!(claude_5h.source, MeterSource::None);
        assert_eq!(claude_5h.observed_at, None);

        store
            .record_limit_hit(
                EngineId::Claude,
                Some(LimitWindowKind::FiveHour),
                "t2".into(),
                Some("2026-09-04T05:00:00Z".into()),
                Usage::default(),
            )
            .await
            .unwrap();
        let hit_meters = store
            .get_meters_at("2026-09-04T02:00:00Z".into())
            .await
            .unwrap();
        let hit_5h = hit_meters
            .iter()
            .find(|m| m.engine == EngineId::Claude && m.window == LimitWindowKind::FiveHour)
            .unwrap();
        assert_eq!(hit_5h.resets_at.as_deref(), Some("2026-09-04T05:00:00Z"));
        assert_eq!(hit_5h.remaining_pct, Some(0.0));
    }

    async fn meter(store: &Store, window: LimitWindowKind) -> MeterState {
        store
            .get_meters_at("2026-09-04T02:00:00Z".into())
            .await
            .unwrap()
            .into_iter()
            .find(|m| m.engine == EngineId::Claude && m.window == window)
            .expect("claude meter")
    }

    #[tokio::test]
    async fn limit_hit_with_a_window_marks_only_that_window() {
        let store = Store::open_in_memory().unwrap();
        store
            .record_limit_hit(
                EngineId::Claude,
                Some(LimitWindowKind::Weekly),
                "2026-09-04T02:00:00Z".into(),
                Some("2026-09-11T02:00:00Z".into()),
                Usage::default(),
            )
            .await
            .unwrap();

        let weekly = meter(&store, LimitWindowKind::Weekly).await;
        assert_eq!(weekly.remaining_pct, Some(0.0));
        assert_eq!(weekly.resets_at.as_deref(), Some("2026-09-11T02:00:00Z"));
        assert_eq!(weekly.source, MeterSource::Vendor);
        assert_eq!(weekly.observed_at.as_deref(), Some("2026-09-04T02:00:00Z"));

        let five_hour = meter(&store, LimitWindowKind::FiveHour).await;
        assert_eq!(
            five_hour.remaining_pct, None,
            "the other window is untouched"
        );
        assert_eq!(five_hour.resets_at, None);
        assert_eq!(five_hour.source, MeterSource::None);
        assert_eq!(five_hour.observed_at, None);

        let others_clean = store
            .get_meters()
            .await
            .unwrap()
            .into_iter()
            .filter(|m| m.engine != EngineId::Claude)
            .all(|m| m.remaining_pct.is_none() && m.resets_at.is_none());
        assert!(others_clean, "other engines are untouched");
    }

    #[tokio::test]
    async fn limit_hit_sets_vendor_source_and_observed_at_to_hit_at() {
        let store = Store::open_in_memory().unwrap();
        store
            .record_limit_hit(
                EngineId::Claude,
                Some(LimitWindowKind::FiveHour),
                "2026-09-04T02:00:00Z".into(),
                Some("2026-09-04T07:00:00Z".into()),
                Usage::default(),
            )
            .await
            .unwrap();

        let five_hour = meter(&store, LimitWindowKind::FiveHour).await;
        assert_eq!(five_hour.remaining_pct, Some(0.0));
        assert_eq!(five_hour.source, MeterSource::Vendor);
        assert_eq!(
            five_hour.observed_at.as_deref(),
            Some("2026-09-04T02:00:00Z")
        );
        assert_eq!(
            five_hour.resets_at.as_deref(),
            Some("2026-09-04T07:00:00Z"),
            "resets_at stays the deadline, not the observation"
        );
    }

    #[tokio::test]
    async fn limit_hit_without_a_window_records_the_hit_and_no_meter() {
        let store = Store::open_in_memory().unwrap();
        let before = store.get_meters().await.unwrap();
        store
            .record_limit_hit(
                EngineId::Claude,
                None,
                "2026-09-04T02:00:00Z".into(),
                Some("2026-09-04T07:00:00Z".into()),
                Usage {
                    input: 1,
                    output: 2,
                    cache: 3,
                },
            )
            .await
            .unwrap();

        assert_eq!(store.get_meters().await.unwrap(), before);

        let (engine, window): (String, Option<String>) = store
            .run(|conn| {
                conn.query_row("SELECT engine, window FROM limit_hits", [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .map_err(Into::into)
            })
            .await
            .unwrap();
        assert_eq!(engine, "claude");
        assert_eq!(window, None, "no window is stored as NULL, never guessed");
    }

    #[test]
    fn v1_database_migrates_to_a_nullable_window() {
        let path = std::env::temp_dir().join(format!("idle-migrate-{}.db", uuid::Uuid::new_v4()));
        // The v1 shape of the two tables the migration touches.
        let v1 = rusqlite::Connection::open(&path).unwrap();
        v1.execute_batch(
            "CREATE TABLE schema_version (id INTEGER PRIMARY KEY CHECK (id = 1), version INTEGER NOT NULL);
             INSERT INTO schema_version (id, version) VALUES (1, 1);
             CREATE TABLE limit_hits (id INTEGER PRIMARY KEY, engine TEXT NOT NULL, window TEXT NOT NULL, hit_at TEXT NOT NULL, resets_at TEXT, used_input INTEGER NOT NULL, used_output INTEGER NOT NULL, used_cache INTEGER NOT NULL);
             INSERT INTO limit_hits (engine, window, hit_at, used_input, used_output, used_cache) VALUES ('claude', 'fiveHour', 't', 1, 2, 3);",
        )
        .unwrap();
        drop(v1);

        let store = Store::open(&path).unwrap();
        drop(store);

        let conn = rusqlite::Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        conn.execute(
            "INSERT INTO limit_hits (engine, window, hit_at, used_input, used_output, used_cache) VALUES ('claude', NULL, 't', 0, 0, 0)",
            [],
        )
        .expect("window is nullable after the migration");
        let kept: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM limit_hits WHERE window = 'fiveHour'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(kept, 1, "existing rows survive the rebuild");

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_database_migrates_meter_source_to_none() {
        let path =
            std::env::temp_dir().join(format!("idle-migrate-v2-{}.db", uuid::Uuid::new_v4()));
        let v2 = rusqlite::Connection::open(&path).unwrap();
        v2.execute_batch(
            "CREATE TABLE schema_version (id INTEGER PRIMARY KEY CHECK (id = 1), version INTEGER NOT NULL);
             INSERT INTO schema_version (id, version) VALUES (1, 2);
             CREATE TABLE meter_state (
                engine TEXT NOT NULL,
                window TEXT NOT NULL,
                used_input INTEGER NOT NULL,
                used_output INTEGER NOT NULL,
                used_cache INTEGER NOT NULL,
                capacity_est INTEGER,
                calibrated INTEGER NOT NULL,
                remaining_pct REAL,
                resets_at TEXT,
                PRIMARY KEY (engine, window)
             );
             INSERT INTO meter_state (engine, window, used_input, used_output, used_cache, capacity_est, calibrated, remaining_pct, resets_at)
             VALUES ('claude', 'fiveHour', 10, 20, 30, 1000, 1, 26.4, '2026-09-04T05:00:00Z');",
        )
        .unwrap();
        drop(v2);

        let store = Store::open(&path).unwrap();
        drop(store);

        let conn = rusqlite::Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let (used_input, remaining, source, observed_at): (i64, f64, String, Option<String>) = conn
            .query_row(
                "SELECT used_input, remaining_pct, source, observed_at FROM meter_state WHERE engine = 'claude' AND window = 'fiveHour'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(used_input, 10, "existing rows survive");
        assert_eq!(remaining, 26.4);
        assert_eq!(source, "none");
        assert_eq!(observed_at, None);

        drop(conn);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn v2_database_with_source_only_gains_observed_at() {
        let path = std::env::temp_dir().join(format!(
            "idle-migrate-v2-partial-{}.db",
            uuid::Uuid::new_v4()
        ));
        let v2 = rusqlite::Connection::open(&path).unwrap();
        v2.execute_batch(
            "CREATE TABLE schema_version (id INTEGER PRIMARY KEY CHECK (id = 1), version INTEGER NOT NULL);
             INSERT INTO schema_version (id, version) VALUES (1, 2);
             CREATE TABLE meter_state (
                engine TEXT NOT NULL,
                window TEXT NOT NULL,
                used_input INTEGER NOT NULL,
                used_output INTEGER NOT NULL,
                used_cache INTEGER NOT NULL,
                capacity_est INTEGER,
                calibrated INTEGER NOT NULL,
                remaining_pct REAL,
                resets_at TEXT,
                source TEXT NOT NULL DEFAULT 'none',
                PRIMARY KEY (engine, window)
             );
             INSERT INTO meter_state (engine, window, used_input, used_output, used_cache, capacity_est, calibrated, remaining_pct, resets_at, source)
             VALUES ('claude', 'fiveHour', 10, 20, 30, 1000, 1, 26.4, '2026-09-04T05:00:00Z', 'none');",
        )
        .unwrap();
        drop(v2);

        let store = Store::open(&path).unwrap();
        drop(store);

        let conn = rusqlite::Connection::open(&path).unwrap();
        let version: i64 = conn
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let names: Vec<String> = conn
            .prepare("PRAGMA table_info(meter_state)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(names.contains(&"source".to_string()));
        assert!(names.contains(&"observed_at".to_string()));
        let (used_input, source, observed_at): (i64, String, Option<String>) = conn
            .query_row(
                "SELECT used_input, source, observed_at FROM meter_state WHERE engine = 'claude' AND window = 'fiveHour'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(used_input, 10, "existing rows survive");
        assert_eq!(source, "none");
        assert_eq!(observed_at, None);

        drop(conn);
        let _ = std::fs::remove_file(&path);
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
        assert!(matches!(err, StoreError::ClaimRejected(_)));
    }

    #[tokio::test]
    async fn get_meters_rolls_vendor_reading_on_injected_now() {
        let store = Store::open_in_memory().unwrap();
        let now = "2026-09-04T00:00:00Z".to_string();
        let event = RunEvent::WindowReading {
            run_id: "r1".into(),
            window: LimitWindowKind::FiveHour,
            utilization: 0.25,
            resets_at: Some("2026-09-04T05:00:00Z".into()),
        };
        store
            .apply_run_event(EngineId::Claude, event, now.clone(), Usage::default())
            .await
            .unwrap();
        let five = store
            .get_meters_at(now)
            .await
            .unwrap()
            .into_iter()
            .find(|m| m.engine == EngineId::Claude && m.window == LimitWindowKind::FiveHour)
            .unwrap();
        assert_eq!(five.remaining_pct, Some(75.0));
        assert_eq!(five.resets_at.as_deref(), Some("2026-09-04T05:00:00Z"));
        assert_eq!(five.source, MeterSource::Vendor);

        let rolled = store
            .get_meters_at("2026-09-04T05:00:00Z".into())
            .await
            .unwrap()
            .into_iter()
            .find(|m| m.engine == EngineId::Claude && m.window == LimitWindowKind::FiveHour)
            .unwrap();
        assert_eq!(rolled.remaining_pct, Some(100.0));
        assert_eq!(rolled.resets_at, None);
        assert_eq!(rolled.source, MeterSource::Vendor);
    }

    #[tokio::test]
    async fn apply_run_event_limit_hit_zeros_that_row_and_records_the_hit() {
        let store = Store::open_in_memory().unwrap();
        let now = "2026-09-04T02:00:00Z".to_string();
        let event = RunEvent::LimitHit {
            run_id: "r1".into(),
            window: Some(LimitWindowKind::FiveHour),
            resets_at: Some("2026-09-04T07:00:00Z".into()),
        };
        let changed = store
            .apply_run_event(EngineId::Claude, event, now.clone(), Usage::default())
            .await
            .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].window, LimitWindowKind::FiveHour);
        assert_eq!(changed[0].remaining_pct, Some(0.0));
        assert_eq!(changed[0].source, MeterSource::Vendor);
        assert_eq!(changed[0].observed_at.as_deref(), Some(now.as_str()));

        let weekly = meter(&store, LimitWindowKind::Weekly).await;
        assert_eq!(weekly.remaining_pct, None);
        assert_eq!(weekly.source, MeterSource::None);

        let hits: i64 = store
            .run(|conn| {
                conn.query_row("SELECT COUNT(*) FROM limit_hits", [], |row| row.get(0))
                    .map_err(Into::into)
            })
            .await
            .unwrap();
        assert_eq!(hits, 1);
    }

    #[tokio::test]
    async fn limit_hit_without_reset_keeps_stored_deadline() {
        let store = Store::open_in_memory().unwrap();
        let now = "2026-09-04T02:00:00Z".to_string();
        store
            .apply_run_event(
                EngineId::Claude,
                RunEvent::WindowReading {
                    run_id: "r1".into(),
                    window: LimitWindowKind::FiveHour,
                    utilization: 0.25,
                    resets_at: Some("2026-09-04T07:00:00Z".into()),
                },
                now.clone(),
                Usage::default(),
            )
            .await
            .unwrap();
        store
            .apply_run_event(
                EngineId::Claude,
                RunEvent::LimitHit {
                    run_id: "r1".into(),
                    window: Some(LimitWindowKind::FiveHour),
                    resets_at: None,
                },
                now.clone(),
                Usage::default(),
            )
            .await
            .unwrap();
        let five = meter(&store, LimitWindowKind::FiveHour).await;
        assert_eq!(five.remaining_pct, Some(0.0));
        assert_eq!(five.source, MeterSource::Vendor);
        assert_eq!(five.resets_at.as_deref(), Some("2026-09-04T07:00:00Z"));
    }

    #[tokio::test]
    async fn refresh_meters_is_silent_when_nothing_changed() {
        let store = Store::open_in_memory().unwrap();
        let changed = store
            .refresh_meters("2026-09-04T00:00:00Z".into())
            .await
            .unwrap();
        assert!(changed.is_empty());
    }

    #[tokio::test]
    async fn refresh_meters_reports_and_persists_the_rolled_window() {
        let store = Store::open_in_memory().unwrap();
        store
            .apply_run_event(
                EngineId::Claude,
                RunEvent::WindowReading {
                    run_id: "r1".into(),
                    window: LimitWindowKind::FiveHour,
                    utilization: 0.25,
                    resets_at: Some("2026-09-04T05:00:00Z".into()),
                },
                "2026-09-04T00:00:00Z".into(),
                Usage::default(),
            )
            .await
            .unwrap();
        let changed = store
            .refresh_meters("2026-09-04T05:00:00Z".into())
            .await
            .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].window, LimitWindowKind::FiveHour);
        assert_eq!(changed[0].remaining_pct, Some(100.0));
        assert_eq!(changed[0].resets_at, None);
        let again = store
            .refresh_meters("2026-09-04T05:00:00Z".into())
            .await
            .unwrap();
        assert!(again.is_empty());
    }
}
