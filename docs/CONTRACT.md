# Contract

Wire shapes for idle-app. Timestamps are RFC3339 strings. JSON uses camelCase.

## Types

| Type            | Shape                                                                             |
| --------------- | --------------------------------------------------------------------------------- |
| EngineId        | `"claude"` \| `"codex"` \| `"antigravity"` \| `"grok"`                            |
| TaskSize        | `"s"` \| `"m"` \| `"l"`                                                           |
| EngineChoice    | `{ type: "auto" }` \| `{ type: "fixed", engine: EngineId }`                       |
| TaskStatus      | `"queued"` \| `"running"` \| `"done"` \| `"failed"` \| `"discarded"`              |
| Task            | `{ id, prompt, folder, size, engine, status, createdAt, updatedAt }`              |
| Usage           | `{ input, output, cache }` (u64, JSON numbers)                                    |
| RunEvent        | tagged on `type`, every variant includes `runId`, see lifecycle                   |
| ExitReason      | `"ok"` \| `"failed"` \| `"limitHit"` \| `"cancelled"` \| `"timeout"`              |
| Run             | `{ id, taskId, engine, startedAt, finishedAt, exitReason, usage, snapshotId }`    |
| LimitWindowKind | `"fiveHour"` \| `"daily"` \| `"weekly"`                                           |
| LimitWindow     | `{ kind, hours }`                                                                 |
| MeterState      | `{ engine, window, used, capacityEst, calibrated, remainingPct (f64), resetsAt }` |
| DetectInfo      | `{ installed, version, signedIn }`                                                |
| EngineStatus    | `{ engine, detect }`                                                              |

`id` values are UUID strings. `folder` is an absolute path. Optional fields are `null` when absent.

## RunEvent lifecycle

Internally tagged on `type`. Every variant carries `runId` so the UI can route up to four concurrent engine streams on one `run_event` channel. A run emits `started`, then zero or more `output`, `usage`, `limitHit`, and `error` events, then exactly one `finished`. `error` is valid mid-stream; a malformed line emits `error` and the run continues.

| `type`     | Fields                              | Terminal? |
| ---------- | ----------------------------------- | --------- |
| `started`  | `runId`                             | no        |
| `output`   | `runId`, `line`                     | no        |
| `usage`    | `runId`, `input`, `output`, `cache` | no        |
| `limitHit` | `runId`, `resetsAt`                 | no        |
| `finished` | `runId`, `ok`                       | yes       |
| `error`    | `runId`, `message`                  | no        |

## IPC commands

Invoke args are the object in Args. Return is the Rust/JSON value. Command and event name strings live in `src-tauri/src/ipc.rs` and are duplicated in `src/types/ipc.ts`. Change both.

| Command       | Args                                                | Returns          |
| ------------- | --------------------------------------------------- | ---------------- |
| `list_tasks`  | (none)                                              | `Task[]`         |
| `add_task`    | `{ prompt, folder, size, engine }`                  | `Task`           |
| `update_task` | `{ id, prompt?, folder?, size?, engine?, status? }` | `Task`           |
| `delete_task` | `{ id }`                                            | `null`           |
| `run_now`     | `{ taskId }`                                        | `Run`            |
| `stop_run`    | `{ runId }`                                         | `null`           |
| `list_runs`   | `{ taskId? }`                                       | `Run[]`          |
| `get_meters`  | (none)                                              | `MeterState[]`   |
| `get_engines` | (none)                                              | `EngineStatus[]` |

## Events

| Event           | Payload        |
| --------------- | -------------- |
| `run_event`     | `RunEvent`     |
| `meter_update`  | `MeterState`   |
| `engine_status` | `EngineStatus` |

## Schema

SQLite tables: `tasks`, `runs`, `meter_state`, `limit_hits`, `schema_version`.

Indexes: `tasks(status)`, `runs(task_id)`, `limit_hits(engine, window)`.

`limit_hits` columns: `id` (INTEGER PRIMARY KEY), `engine`, `window`, `hit_at`, `resets_at`, `used_input`, `used_output`, `used_cache`. Append-only. No composite key on `(engine, window, hit_at)`: sub-second duplicate hits on the same window are allowed. Never prune.

`schema_version` is one row: `id INTEGER PRIMARY KEY CHECK (id = 1)`, `version` starts at `1`. Reapplying the schema uses `INSERT OR IGNORE` and `CREATE IF NOT EXISTS`, so the version table stays one row.

Usage on `runs` and `meter_state` is stored as `used_input`, `used_output`, `used_cache`.

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
