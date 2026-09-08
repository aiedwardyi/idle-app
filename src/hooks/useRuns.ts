import { useCallback, useEffect, useRef, useState } from "react";
import type { EngineId, Task } from "../types";
import { listenRunEvent, runNow, stopRun } from "../types/ipc";
import { message } from "../lib/errors";

export type ActiveRun = { runId: string; taskId: string; engine: EngineId };

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
export function useRuns(onSettled: () => void) {
  const [active, setActive] = useState<ActiveRun[]>([]);
  const [error, setError] = useState<string | null>(null);

  // The listener is registered once, so it must not close over a stale
  // callback: the ref is what keeps it pointing at the current one. Written
  // in an effect rather than in the render body — a ref update during render
  // is exactly the thing that makes a component miss an update.
  const settled = useRef(onSettled);
  useEffect(() => {
    settled.current = onSettled;
  }, [onSettled]);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const stop = await listenRunEvent((event) => {
          if (event.type === "finished" || event.type === "error") {
            setActive((current) =>
              current.filter((run) => run.runId !== event.runId),
            );
            if (event.type === "error") setError(event.message);
            settled.current();
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
      setActive((current) => [
        ...current,
        { runId: run.id, taskId: run.taskId, engine: run.engine },
      ]);
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

  return { active, error, start, stop };
}
