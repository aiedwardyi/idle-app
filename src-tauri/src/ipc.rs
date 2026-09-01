//! IPC names only. Command handlers land in a later PR.

/// No args. Returns `Vec<Task>`.
pub const LIST_TASKS: &str = "list_tasks";
/// Args `{ prompt, folder, size, engine }`. Returns `Task`.
pub const ADD_TASK: &str = "add_task";
/// Args `{ id, prompt?, folder?, size?, engine?, status? }`. Returns `Task`.
pub const UPDATE_TASK: &str = "update_task";
/// Args `{ id }`. Returns null.
pub const DELETE_TASK: &str = "delete_task";
/// Args `{ taskId }`. Returns `Run`.
pub const RUN_NOW: &str = "run_now";
/// Args `{ runId }`. Returns null.
pub const STOP_RUN: &str = "stop_run";
/// Args `{ taskId? }`. Returns `Vec<Run>`.
pub const LIST_RUNS: &str = "list_runs";
/// No args. Returns `Vec<MeterState>`.
pub const GET_METERS: &str = "get_meters";
/// No args. Returns `Vec<EngineStatus>`.
pub const GET_ENGINES: &str = "get_engines";

/// Payload `RunEvent`.
pub const RUN_EVENT: &str = "run_event";
/// Payload `MeterState`.
pub const METER_UPDATE: &str = "meter_update";
/// Payload `EngineStatus`.
pub const ENGINE_STATUS: &str = "engine_status";
