import { useCallback, useEffect, useState } from "react";
import type { EngineId } from "../types";
import { listenRunEvent, runNext, stopRun } from "../types/ipc";
import { message } from "../lib/errors";

export type ActiveRun = {
  runId: string;
  taskId: string;
  engine: EngineId;
  startedAt: string;
  lastLine: string | null;
};

/**
 * Runs started from the meters screen, keyed by runId so up to four engine
 * streams on one `run_event` channel never cross-wire. The row Play button
 * reflects this map, never a local boolean.
 */
export function useRuns() {
  const [runs, setRuns] = useState<Record<string, ActiveRun>>({});
  const [starting, setStarting] = useState<Partial<Record<EngineId, boolean>>>(
    {},
  );
  const [stopping, setStopping] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        const stop = await listenRunEvent((event) => {
          if (event.type === "output") {
            setRuns((current) => {
              const found = current[event.runId];
              if (found === undefined) return current;
              return {
                ...current,
                [event.runId]: { ...found, lastLine: event.line },
              };
            });
          } else if (event.type === "finished") {
            setRuns((current) => {
              if (!(event.runId in current)) return current;
              const next = { ...current };
              delete next[event.runId];
              return next;
            });
          }
        });
        if (live) unlisten = stop;
        else stop();
      } catch {
        // No channel means no live lines; runs started here still show.
      }
    })();

    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  const start = useCallback(async (engine: EngineId) => {
    setStarting((current) => ({ ...current, [engine]: true }));
    try {
      const run = await runNext({ engine });
      setRuns((current) => ({
        ...current,
        [run.id]: {
          runId: run.id,
          taskId: run.taskId,
          engine: run.engine,
          startedAt: run.startedAt,
          lastLine: null,
        },
      }));
      setError(null);
    } catch (caught) {
      setError(message(caught));
    } finally {
      setStarting((current) => ({ ...current, [engine]: false }));
    }
  }, []);

  const stop = useCallback(async (runId: string) => {
    setStopping((current) => ({ ...current, [runId]: true }));
    try {
      await stopRun({ runId });
      setError(null);
    } catch (caught) {
      setError(message(caught));
    } finally {
      setStopping((current) => {
        const next = { ...current };
        delete next[runId];
        return next;
      });
    }
  }, []);

  return { runs, starting, stopping, error, start, stop };
}
