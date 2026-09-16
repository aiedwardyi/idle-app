import type { Task } from "../types";
import type { ActiveRun } from "../hooks/useRuns";
import { ENGINE_LABEL, ENGINE_ORDER } from "../lib/engines";
import { formatElapsed } from "../lib/meters";

type Props = {
  runs: ActiveRun[];
  tasks: Task[] | null;
  now: Date;
  stopping: Record<string, boolean>;
  onStop: (runId: string) => void;
};

/**
 * One compact strip per active run, above the meters. Engine, task prompt (or
 * task identity when the row is gone), elapsed MP3 time, latest output line,
 * and a Stop that affects only that runId.
 */
export function NowPlaying({ runs, tasks, now, stopping, onStop }: Props) {
  if (runs.length === 0) return null;

  const ordered = [...runs].sort(
    (a, b) => ENGINE_ORDER.indexOf(a.engine) - ENGINE_ORDER.indexOf(b.engine),
  );
  const byId = new Map((tasks ?? []).map((task) => [task.id, task]));

  return (
    <div className="nowplaying">
      {ordered.map((run) => {
        const label = ENGINE_LABEL[run.engine];
        const task = byId.get(run.taskId);
        return (
          <div key={run.runId} className="np-row">
            <div className="np-top">
              <span className="np-engine">{label}</span>
              <span className="np-elapsed">
                {formatElapsed(run.startedAt, now)}
              </span>
              <button
                type="button"
                className="np-stop"
                aria-label={`Stop ${label} run`}
                disabled={stopping[run.runId] ?? false}
                onClick={() => onStop(run.runId)}
              >
                Stop
              </button>
            </div>
            <div className="np-task">
              {task?.prompt ?? `task ${run.taskId.slice(0, 8)}`}
            </div>
            <div className="np-line">{run.lastLine ?? "Starting…"}</div>
          </div>
        );
      })}
    </div>
  );
}
