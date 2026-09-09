# Contract

Wire shapes for idle-app. Timestamps are RFC3339 strings. JSON uses camelCase.

## Types

| Type            | Shape                                                                                                 |
| --------------- | ----------------------------------------------------------------------------------------------------- |
| EngineId        | `"claude"` \| `"codex"` \| `"antigravity"` \| `"grok"`                                                |
| TaskSize        | `"s"` \| `"m"` \| `"l"`                                                                               |
| EngineChoice    | `{ type: "auto" }` \| `{ type: "fixed", engine: EngineId }`                                           |
| TaskStatus      | `"queued"` \| `"running"` \| `"done"` \| `"failed"` \| `"discarded"`                                  |
| Task            | `{ id, prompt, folder, size, engine, status, createdAt, updatedAt }`                                  |
| Usage           | `{ input, output, cache }` (u64, JSON numbers)                                                        |
| RunEvent        | tagged on `type`, every variant includes `runId`, see lifecycle                                       |
| ExitReason      | `"ok"` \| `"failed"` \| `"limitHit"` \| `"cancelled"` \| `"timeout"`                                  |
| Run             | `{ id, taskId, engine, startedAt, finishedAt, exitReason, usage, snapshotId }`                        |
| LimitWindowKind | `"fiveHour"` \| `"daily"` \| `"weekly"`                                                               |
| LimitWindow     | `{ kind, hours }`                                                                                     |
| MeterSource     | `"vendor"` \| `"estimate"` \| `"none"`                                                                |
| MeterState      | `{ engine, window, used, capacityEst, calibrated, remainingPct (f64), resetsAt, source, observedAt }` |
| DetectInfo      | `{ installed, version, signedIn }`                                                                    |
| Schedule        | `{ enabled, quietStart, quietEnd, reservePct, idleMinutes, maxConcurrent }`                           |
| SchedulerStatus | `{ engine, state, reason, until }`                                                                    |
| EngineStatus    | `{ engine, detect }`                                                                                  |

`id` values are UUID strings. `folder` is an absolute path. Optional fields are `null` when absent.

`MeterSource` says where `remainingPct` came from: `vendor` is the vendor payload's own number, `estimate` is computed from token sums against a learned capacity, `none` means no percent is known and the UI shows a dash.

## Schedule

Defaults: disabled, quiet hours `23:00` to `07:00`, reserve 25%, idle 10 minutes, max concurrent 2. `set_schedule` rejects malformed `HH:MM`, reserve outside 0..95, idle outside 0..120, and concurrency outside 1..4. It never clamps.

Quiet hours are the operating window, not a mute window: `quietStart` and `quietEnd` bound the hours in which auto is permitted to start runs, and auto reports `quietHours` outside them. Both are local wall-clock `HH:MM`, start inclusive and end exclusive. The window wraps midnight, so the `23:00` to `07:00` default is one valid window and not an error. Equal endpoints mean the window is always open, never always closed. Windows probes last input with wrapping tick arithmetic. Other platforms report Unknown, which passes the idle gate. Turning auto off prevents new starts and leaves active runs alone.

Every known meter window must have `remainingPct >= reservePct + margin`: S 5, M 15, L 30. `source: none` does not block. No reset-soon gate. S/M/L map to CLI effort low/medium/high and retain their existing timeouts.

FIFO is by `createdAt`, with `id` breaking ties. Auto resolves to Claude. Missing adapters, missing binaries, and signed-out engines wait. Detection is cached for five minutes, and is not probed at all while auto is off or no task is queued. One unfinished run per engine, and at most `maxConcurrent` overall, including manual starts. Click, `run_next`, and auto share one start path and atomic database claim. `run_next` ignores auto, quiet hours, idle, and reserve, but honors concurrency and cooldown, as does the click.

`SchedulerStatus.state` is `off | waiting | running | paused`. `reason` is `quietHours | notIdle | busy | reserve | cooldown | noTasks | engineUnavailable | ready`. `until` is the cooldown's RFC3339 deadline or null. Active engines report running/busy even with auto off; otherwise off uses noTasks. A task that clears every gate reports waiting/ready: never waiting/noTasks, and never running/busy before its run exists. Status is emitted once per changed engine, including the first tick. A five-second loop skips missed ticks after sleep.

A database owner holds an OS file lock before startup reconciliation. Another live instance cannot open the same database. Deleting a task with an unfinished run returns an error.

A limit-hit run requeues its task on the first two hits; the third fails it. The engine pauses until the hit's reset, or hit time plus 60 minutes if absent. Cooldowns are derived from append-only `limit_hits` across restart. No other terminal outcome retries automatically.

A vendor zero with no deadline expires. A `source: vendor` window at 0% whose `resetsAt` is null reverts to `source: none` with a null `remainingPct` once its `observedAt` is older than the fallback cooldown. It can never roll over, so holding it would block the reserve gate forever and strand the requeued task. This is not a guess and not a rollover: we stop asserting a stale zero we can no longer justify, the UI shows a dash, and the next run supplies a fresh reading. The expiry and the fallback cooldown share one constant so they cannot drift apart.

Only a queued task is claimable. Every start path rejects any other status, so the third limit hit cannot be silently re-run.

| From                                    | Trigger                                 | To        |
| --------------------------------------- | --------------------------------------- | --------- |
| queued                                  | Claimed by any start path               | running   |
| running                                 | ok                                      | done      |
| running                                 | limitHit, first or second run hit       | queued    |
| running                                 | limitHit, third or later run hit        | failed    |
| running                                 | failed or timeout                       | failed    |
| running                                 | cancelled                               | discarded |
| failed, done, or discarded              | Retry must requeue before `run_now`     | queued    |
| any task with an unfinished run at boot | Reconcile run as failed, finishedAt now | failed    |

## RunEvent lifecycle

Internally tagged on `type`. Every variant carries `runId` so the UI can route up to four concurrent engine streams on one `run_event` channel. A run emits `started`, then zero or more `output`, `usage`, `limitHit`, `windowReading`, and `error` events, then exactly one `finished`. `error` is valid mid-stream; a malformed line emits `error` and the run continues. `windowReading` is the vendor-stated fill level of one window; never derived, never guessed.

`limitHit.window` is the `LimitWindowKind` the vendor payload named as exhausted, or `null` when the payload did not name one. `null` is legal and is never a guess. A consumer must record the hit and leave every meter alone: no bucket may be picked by default, and the hit must not be spread across an engine's windows.

| `type`          | Fields                                       | Terminal? |
| --------------- | -------------------------------------------- | --------- |
| `started`       | `runId`                                      | no        |
| `output`        | `runId`, `line`                              | no        |
| `usage`         | `runId`, `input`, `output`, `cache`          | no        |
| `limitHit`      | `runId`, `window`, `resetsAt`                | no        |
| `windowReading` | `runId`, `window`, `utilization`, `resetsAt` | no        |
| `finished`      | `runId`, `ok`                                | yes       |
| `error`         | `runId`, `message`                           | no        |

## IPC commands

Invoke args are the object in Args. Return is the Rust/JSON value. Command and event name strings live in `src-tauri/src/ipc.rs` and are duplicated in `src/types/ipc.ts`. Change both.

| Command               | Args                                                | Returns             |
| --------------------- | --------------------------------------------------- | ------------------- |
| `list_tasks`          | (none)                                              | `Task[]`            |
| `add_task`            | `{ prompt, folder, size, engine }`                  | `Task`              |
| `update_task`         | `{ id, prompt?, folder?, size?, engine?, status? }` | `Task`              |
| `delete_task`         | `{ id }`                                            | `null`              |
| `run_now`             | `{ taskId }`                                        | `Run`               |
| `stop_run`            | `{ runId }`                                         | `null`              |
| `list_runs`           | `{ taskId? }`                                       | `Run[]`             |
| `get_meters`          | (none)                                              | `MeterState[]`      |
| `get_engines`         | (none)                                              | `EngineStatus[]`    |
| `get_schedule`        | (none)                                              | `Schedule`          |
| `set_schedule`        | `{ schedule }`                                      | `null`              |
| `get_schedule_status` | (none)                                              | `SchedulerStatus[]` |
| `run_next`            | `{ engine }`                                        | `Run`               |

## Events

| Event             | Payload           |
| ----------------- | ----------------- |
| `run_event`       | `RunEvent`        |
| `meter_update`    | `MeterState`      |
| `engine_status`   | `EngineStatus`    |
| `schedule_status` | `SchedulerStatus` |

## Schema

SQLite tables: `tasks`, `runs`, `meter_state`, `limit_hits`, `schedule`, `schema_version`.

Indexes: `tasks(status)`, `runs(task_id)`, `limit_hits(engine, window)`, `limit_hits(julianday(COALESCE(resets_at, hit_at)))`. Cooldown reads use the time index to exclude expired history without pruning.

`limit_hits` columns: `id` (INTEGER PRIMARY KEY), `engine`, `window`, `hit_at`, `resets_at`, `used_input`, `used_output`, `used_cache`. `window` is nullable and holds `limitHit.window`, so a hit with no window evidence is still calibration ground truth. Append-only. No composite key on `(engine, window, hit_at)`: sub-second duplicate hits on the same window are allowed. Never prune.

`schema_version` is one row: `id INTEGER PRIMARY KEY CHECK (id = 1)`, `version` is `4`. Reapplying the schema uses `INSERT OR IGNORE` and `CREATE IF NOT EXISTS`, so the version table stays one row. An older database is upgraded on open by `store::migrate`, which runs the steps above its recorded version and writes the new one.

Usage on `runs` and `meter_state` is stored as `used_input`, `used_output`, `used_cache`. `meter_state` also stores `source TEXT NOT NULL DEFAULT 'none'` and `observed_at TEXT`.

`schedule` has `id INTEGER PRIMARY KEY CHECK (id = 1)` and nullable `config TEXT` containing Schedule JSON. An absent row or null config uses defaults. Migration checks the table and config column independently before advancing the version.

## Default windows

| Engine      | Windows                |
| ----------- | ---------------------- |
| Claude      | FiveHour 5, Weekly 168 |
| Codex       | FiveHour 5, Weekly 168 |
| Antigravity | Daily 24               |
| Grok        | Weekly 168             |

`default_windows(engine)` in Rust returns these. Hours are the window length.

## Engine trait

`detect`, `install`, and `login` are async (`async_trait`, so `dyn Engine` stays object-safe). `run` returns `EngineRun`: the `RunEvent` stream plus `kill()` and `wait() -> ExitReason`, the same shape as the Runner's handle. A bare stream cannot report `limitHit`, `cancelled`, or `timeout`, and the meter needs `limitHit`. `id` and `windows` stay sync. `RunCtx.cwd` is `PathBuf`. `Task.folder` stays a String because it crosses the IPC wire.

Adapters translate stdout through an `EventMapper`; the shared pump in `EngineRun` passes Runner `started` and `error` events through, feeds every JSON stdout line to the mapper, and closes with the mapper's final events plus one `finished`. A well-formed line the mapper does not recognise is an `error` carrying the raw line.

## Hard rules

- Every run is an unmodified official CLI as a subprocess.
- All subprocesses go through `Runner::spawn`. Never call `Command::new` directly.
- The app never holds a credential and never calls a model.
- Never modify or vendor a CLI binary. Official installer only.
- No network calls from the app except the update check.
- Webview CSP is `default-src 'self'`; `connect-src` adds only the IPC origins (`ipc:`, and `http://ipc.localhost` on Windows), which carry commands, not network traffic. `style-src 'self' 'unsafe-inline'` is listed because the UI uses inline style attributes; that directive is deliberately permissive. `img-src 'self' data:` is listed because Vite inlines assets under its size threshold; a future icon would break silently. Tauri serves that header from the asset protocol, so `tauri dev` on the Vite URL enforces no CSP - verify against a build. Only Windows/WebView2 is verified; macOS and Linux are untested.
- Detect reads exit codes only, never files.
- Do not prune `limit_hits`.
- UI screens have 3 controls or fewer.
- Scrub these env vars from every subprocess: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CODEX_API_KEY`, `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `XAI_API_KEY`.
