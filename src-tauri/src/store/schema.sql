CREATE TABLE IF NOT EXISTS schema_version (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    version INTEGER NOT NULL
);

INSERT OR IGNORE INTO schema_version (id, version) VALUES (1, 1);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    prompt TEXT NOT NULL,
    folder TEXT NOT NULL,
    size TEXT NOT NULL,
    engine TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks (status);

CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks (id),
    engine TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    exit_reason TEXT,
    used_input INTEGER NOT NULL,
    used_output INTEGER NOT NULL,
    used_cache INTEGER NOT NULL,
    snapshot_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_runs_task_id ON runs (task_id);

CREATE TABLE IF NOT EXISTS meter_state (
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

-- Calibration ground truth. Never prune this table.
CREATE TABLE IF NOT EXISTS limit_hits (
    engine TEXT NOT NULL,
    window TEXT NOT NULL,
    hit_at TEXT NOT NULL,
    resets_at TEXT,
    used_input INTEGER NOT NULL,
    used_output INTEGER NOT NULL,
    used_cache INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_limit_hits_engine_window ON limit_hits (engine, window);
