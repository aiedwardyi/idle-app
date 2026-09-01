# Contract

Wire shapes for idle-app. Timestamps are RFC3339 strings. JSON uses camelCase.

## Types

| Type            | Shape                                                                          |
| --------------- | ------------------------------------------------------------------------------ |
| EngineId        | `"claude"` \| `"codex"` \| `"antigravity"` \| `"grok"`                         |
| TaskSize        | `"s"` \| `"m"` \| `"l"`                                                        |
| EngineChoice    | `{ type: "auto" }` \| `{ type: "fixed", engine: EngineId }`                    |
| TaskStatus      | `"queued"` \| `"running"` \| `"done"` \| `"failed"` \| `"discarded"`           |
| Task            | `{ id, prompt, folder, size, engine, status, createdAt, updatedAt }`           |
| Usage           | `{ input, output, cache }` (u64, JSON numbers)                                 |
| RunEvent        | tagged, see lifecycle                                                          |
| ExitReason      | `"ok"` \| `"failed"` \| `"limitHit"` \| `"cancelled"` \| `"timeout"`           |
| Run             | `{ id, taskId, engine, startedAt, finishedAt, exitReason, usage, snapshotId }` |
| LimitWindowKind | `"fiveHour"` \| `"daily"` \| `"weekly"`                                        |
| LimitWindow     | `{ kind, hours }`                                                              |
| MeterState      | `{ engine, window, used, capacityEst, calibrated, remainingPct, resetsAt }`    |
| DetectInfo      | `{ installed, version, signedIn }`                                             |
| EngineStatus    | `{ engine, detect }`                                                           |

`id` values are UUID strings. `folder` is an absolute path. Optional fields are `null` when absent.

## RunEvent lifecycle

Internally tagged on `type`. A run emits `started`, then zero or more `output` and `usage` events, then exactly one of `limitHit`, `finished`, or `error`.

| `type`     | Fields                                         |
| ---------- | ---------------------------------------------- |
| `started`  | `runId`                                        |
| `output`   | `line`                                         |
| `usage`    | `input`, `output`, `cache` (flattened `Usage`) |
| `limitHit` | `resetsAt`                                     |
| `finished` | `ok`                                           |
| `error`    | `message`                                      |

## IPC commands

Invoke args are the object in Args. Return is the Rust/JSON value.

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

`limit_hits` columns: `engine`, `window`, `hit_at`, `resets_at`, `used_input`, `used_output`, `used_cache`. This table is calibration ground truth and is never pruned.

`schema_version` starts at `1`.

Usage on `runs` and `meter_state` is stored as `used_input`, `used_output`, `used_cache`.

## Default windows

| Engine      | Windows                |
| ----------- | ---------------------- |
| Claude      | FiveHour 5, Weekly 168 |
| Codex       | FiveHour 5, Weekly 168 |
| Antigravity | Daily 24               |
| Grok        | Weekly 168             |

`default_windows(engine)` in Rust returns these. Hours are the window length.

## Hard rules

- Every run is an unmodified official CLI as a subprocess.
- All subprocesses go through `Runner::spawn`. Never call `Command::new` directly.
- The app never holds a credential and never calls a model.
- Never modify or vendor a CLI binary. Official installer only.
- No network calls from the app except the update check.
- Detect reads exit codes only, never files.
- Do not prune `limit_hits`.
- UI screens have 3 controls or fewer.
- Scrub these env vars from every subprocess: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CODEX_API_KEY`, `GEMINI_API_KEY`, `GOOGLE_API_KEY`, `XAI_API_KEY`.
