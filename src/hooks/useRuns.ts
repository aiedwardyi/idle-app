import { useCallback, useEffect, useRef, useState } from "react";
import type { Run, Task } from "../types";
import { listenRunEvent, listRuns, runNow, stopRun } from "../types/ipc";
import { message } from "../lib/errors";

/**
 * A run this window started that has not exited yet. The whole Run is kept,
 * not just its id: the results strip draws live work in the same shape as
 * settled work, and a synthetic stand-in would be a second source of truth.
 */
export type ActiveRun = Run;

/**
 * A finished run's transcript tail, kept live while this window is open.
 * The store keeps the run record, not its output, so anything printed before
 * this mount is already gone: the tail starts empty and grows with `output`
 * events. Capped — a run can stream thousands of lines overnight.
 */
const MAX_LINES = 200;

/**
 * The runs this window has started, and the live ones among them.
 *
 * `run_now` takes one task and returns one Run; there is deliberately no
 * scheduler here. Nothing picks a next task, so a finished run leaves the
 * engine idle until someone presses play again — which is the truth about what
 * the app does today rather than a queue that quietly isn't being worked.
 *
 * A run's terminal state lives in the store, not here: the backend flips the
 * task to done / failed / discarded after the process exits, so `finished` and
 * `error` events call back for a re-read instead of guessing the new status.
 */
export function useRuns(onSettled: () => void, tasks: Task[] | null) {
  const [active, setActive] = useState<ActiveRun[]>([]);
  // null = first list_runs has not returned; [] = no finished runs.
  const [finished, setFinished] = useState<Run[] | null>(null);
  const [tails, setTails] = useState<Record<string, string[]>>({});
  const [error, setError] = useState<string | null>(null);

  // The listener is registered once, so it must not close over a stale
  // callback: the ref is what keeps it pointing at the current one. Written
  // in an effect rather than in the render body — a ref update during render
  // is exactly the thing that makes a component miss an update.
  const settled = useRef(onSettled);
  useEffect(() => {
    settled.current = onSettled;
  }, [onSettled]);

  // Overnight runs finished before this window opened, so no event will ever
  // announce them — they load once, up front. Spelled out rather than calling
  // refreshResults(): a bare setState in an effect body is a cascading render,
  // and the mounted guard belongs to this effect, not the shared callback.
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const runs = await listRuns();
        if (live) setFinished(runs.filter((run) => run.finishedAt !== null));
      } catch {
        // No history is not an error banner: meters and queue still stand.
        if (live) setFinished([]);
      }
    })();
    return () => {
      live = false;
    };
  }, []);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const stop = await listenRunEvent((event) => {
          if (event.type === "finished" || event.type === "error") {
            setActive((current) =>
              current.filter((run) => run.id !== event.runId),
            );
            if (event.type === "error") setError(event.message);
            // `error` is mid-stream — the run continues — so only a terminal
            // `finished` settles the task and refreshes its overnight row: the
            // row re-reads itself, then the task list follows.
            if (event.type === "finished") {
              const runId = event.runId;
              void (async () => {
                try {
                  const runs = await listRuns();
                  const done = runs.find((run) => run.id === runId);
                  if (done !== undefined && done.finishedAt !== null) {
                    setFinished((current) => {
                      const rest = (current ?? []).filter(
                        (run) => run.id !== done.id,
                      );
                      return [...rest, done];
                    });
                  }
                } catch {
                  // History stays as it was; the task re-read still happens.
                }
                settled.current();
              })();
            } else settled.current();
          } else if (event.type === "output") {
            const line = event.line;
            setTails((current) => ({
              ...current,
              [event.runId]: [...(current[event.runId] ?? []), line].slice(
                -MAX_LINES,
              ),
            }));
          }
        });
        if (live) unlisten = stop;
        else stop();
      } catch {
        // No event channel means no live updates; starting a run still works.
      }
    })();

    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  const start = useCallback(async (task: Task) => {
    try {
      const run = await runNow({ taskId: task.id });
      setActive((current) => [...current, run]);
      setError(null);
      // The task is `running` in the store now, so the list is out of date.
      settled.current();
    } catch (caught) {
      setError(message(caught));
    }
  }, []);

  const stop = useCallback(async (runId: string) => {
    try {
      await stopRun({ runId });
      setError(null);
    } catch (caught) {
      setError(message(caught));
    }
  }, []);

  // Task id -> prompt, derived from the queue each render rather than stored:
  // overnight runs outlive their tasks in the visible list, so the rows read
  // it off whatever the store last returned. A task removed since keeps its
  // last-known name only if it is still in the list — otherwise "Removed task".
  const prompts = Object.fromEntries(
    (tasks ?? []).map((task) => [task.id, task.prompt]),
  ) as Record<string, string>;

  // What the results strip reads: live work and settled work in one list, so
  // a run moves from amber to green in place rather than disappearing from one
  // collection and reappearing in another.
  const results = [...active, ...(finished ?? [])];

  return { active, finished, results, prompts, tails, error, start, stop };
}
