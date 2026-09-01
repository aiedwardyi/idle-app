// Command and event strings are duplicated from src-tauri/src/ipc.rs. Change both.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { EngineChoice } from "./generated/EngineChoice";
import type { EngineStatus } from "./generated/EngineStatus";
import type { MeterState } from "./generated/MeterState";
import type { Run } from "./generated/Run";
import type { RunEvent } from "./generated/RunEvent";
import type { Task } from "./generated/Task";
import type { TaskSize } from "./generated/TaskSize";
import type { TaskStatus } from "./generated/TaskStatus";

export type AddTask = {
  prompt: string;
  folder: string;
  size: TaskSize;
  engine: EngineChoice;
};

export type UpdateTask = {
  id: string;
  prompt?: string;
  folder?: string;
  size?: TaskSize;
  engine?: EngineChoice;
  status?: TaskStatus;
};

export function listTasks(): Promise<Task[]> {
  return invoke("list_tasks");
}

export function addTask(args: AddTask): Promise<Task> {
  return invoke("add_task", args);
}

export function updateTask(args: UpdateTask): Promise<Task> {
  return invoke("update_task", args);
}

export function deleteTask(args: { id: string }): Promise<void> {
  return invoke("delete_task", args);
}

export function runNow(args: { taskId: string }): Promise<Run> {
  return invoke("run_now", args);
}

export function stopRun(args: { runId: string }): Promise<void> {
  return invoke("stop_run", args);
}

export function listRuns(args: { taskId?: string } = {}): Promise<Run[]> {
  return invoke("list_runs", args);
}

export function getMeters(): Promise<MeterState[]> {
  return invoke("get_meters");
}

export function getEngines(): Promise<EngineStatus[]> {
  return invoke("get_engines");
}

export function listenRunEvent(
  handler: (payload: RunEvent) => void,
): Promise<UnlistenFn> {
  return listen<RunEvent>("run_event", (event) => handler(event.payload));
}

export function listenMeterUpdate(
  handler: (payload: MeterState) => void,
): Promise<UnlistenFn> {
  return listen<MeterState>("meter_update", (event) => handler(event.payload));
}

export function listenEngineStatus(
  handler: (payload: EngineStatus) => void,
): Promise<UnlistenFn> {
  return listen<EngineStatus>("engine_status", (event) =>
    handler(event.payload),
  );
}
