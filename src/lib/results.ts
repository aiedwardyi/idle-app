import type { ExitReason, Run } from "../types";

/**
 * Three states, because three is what a colour can say: something is still
 * working, something finished clean, something wants a person. Every exit
 * reason that is not `ok` lands in `issue` — a failure, a timeout, a limit
 * and a stop all leave the same job undone, and the strip spells out which
 * one it was beside the colour.
 */
export type ResultState = "ongoing" | "done" | "issue";

/** The word beside the colour — rule 9, hue never carries state alone. */
export const RESULT_LABEL: Record<ExitReason, string> = {
  ok: "done",
  failed: "failed",
  limitHit: "limit hit",
  cancelled: "stopped",
  timeout: "timed out",
};

export const ONGOING_LABEL = "running";

/**
 * A run is ongoing until the store gives it both an end and a reason. Either
 * one missing means the process has not settled, so it is still live work
 * rather than a result to read.
 */
export function resultState(run: Run): ResultState {
  if (run.finishedAt === null || run.exitReason === null) return "ongoing";
  return run.exitReason === "ok" ? "done" : "issue";
}

export function resultWord(run: Run): string {
  if (run.exitReason === null || resultState(run) === "ongoing")
    return ONGOING_LABEL;
  return RESULT_LABEL[run.exitReason];
}

/** "2h 14m" overnight, "41m", "3d 2h"; null while a run is still going. */
export function resultDuration(run: Run): string | null {
  if (run.finishedAt === null) return null;
  const ms = Date.parse(run.finishedAt) - Date.parse(run.startedAt);
  if (Number.isNaN(ms) || ms < 0) return null;
  const minutes = Math.max(1, Math.round(ms / 60_000));
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  if (hours < 48)
    return hours < 24 ? `${hours}h` : `${hours}h ${minutes % 60}m`;
  return `${Math.floor(hours / 24)}d ${hours % 24}h`;
}

/** Runs keyed by the task they belong to, for the per-task strips. */
export function runsByTask(runs: Run[]): Record<string, Run[]> {
  const byTask: Record<string, Run[]> = {};
  for (const run of runs) (byTask[run.taskId] ??= []).push(run);
  return byTask;
}

/**
 * The run a task's strip speaks for: live work if there is any, because that
 * is the only part still changing, otherwise the one that finished last.
 * A task run twice shows its latest attempt, not its first.
 */
export function latestRun(runs: Run[]): Run | null {
  if (runs.length === 0) return null;
  const live = runs.find((run) => resultState(run) === "ongoing");
  if (live !== undefined) return live;
  return runs.reduce((newest, run) =>
    (run.finishedAt ?? "") > (newest.finishedAt ?? "") ? run : newest,
  );
}
