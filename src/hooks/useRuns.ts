import { useCallback, useEffect, useRef, useState } from "react";
import type { EngineId, Run, RunEvent } from "../types";
import { listRuns, listenRunEvent, runNext, stopRun } from "../types/ipc";
import { message } from "../lib/errors";

export type ActiveRun = {
  runId: string;
  taskId: string;
  engine: EngineId;
  startedAt: string;
  lastLine: string | null;
};

const toActiveRun = (run: Run): ActiveRun => ({
  runId: run.id,
  taskId: run.taskId,
  engine: run.engine,
  startedAt: run.startedAt,
  lastLine: null,
});

const isLifecycle = (event: RunEvent): boolean =>
  event.type === "output" ||
  event.type === "error" ||
  event.type === "finished";

/**
 * Runs keyed by runId so up to four engine streams on one `run_event`
 * channel never cross-wire. The row Play button reflects this map, never a
 * local boolean.
 *
 * The listener is registered before the `listRuns` snapshot so nothing
 * between them is lost: events arriving during hydration are buffered and
 * replayed in order. Events for unknown run IDs are buffered the same way —
 * a `finished` that lands before `runNext` resolves must not re-add the run —
 * and reconciled against `listRuns` so scheduler-started runs appear.
 *
 * Per the contract, `error` is mid-stream: a malformed line emits `error`
 * and the run continues. It surfaces in `error`, never removes the run.
 */
export function useRuns() {
  const [runs, setRuns] = useState<Record<string, ActiveRun>>({});
  const [starting, setStarting] = useState<Partial<Record<EngineId, boolean>>>(
    {},
  );
  const [stopping, setStopping] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  // Sync mirrors so event handlers and callbacks see the latest state
  // without waiting for a re-render.
  const runsRef = useRef<Record<string, ActiveRun>>({});
  const stoppingRef = useRef<Set<string>>(new Set());
  const readyRef = useRef(false);
  // Lifecycle events for runs not yet registered, in arrival order.
  const pendingRef = useRef<Map<string, RunEvent[]>>(new Map());
  // Run IDs that finished before registration. Checked before any insert;
  // IDs are never reused, so a marker is never wrong.
  const deadRef = useRef<Set<string>>(new Set());
  const reconcilingRef = useRef(false);
  const reconcileDirtyRef = useRef(false);

  useEffect(() => {
    // A copy: the ref must never alias the state object, or an in-place
    // write would mutate state outside its updater.
    runsRef.current = { ...runs };
  }, [runs]);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;
    const hydrating: RunEvent[] = [];
    let hydratingFlag = true;

    const removeRun = (runId: string) => {
      pendingRef.current.delete(runId);
      // The marker stays so a late event cannot resurrect the run.
      deadRef.current.add(runId);
      // Copy, never mutate: the ref can alias the state object, and an
      // in-place delete would make the updater below see a run-free map
      // and bail out, leaving the strip on screen.
      const kept = { ...runsRef.current };
      delete kept[runId];
      runsRef.current = kept;
      setRuns((current) => {
        if (!(runId in current)) return current;
        const next = { ...current };
        delete next[runId];
        return next;
      });
      if (stoppingRef.current.delete(runId)) {
        setStopping((current) => {
          if (!(runId in current)) return current;
          const next = { ...current };
          delete next[runId];
          return next;
        });
      }
    };

    const applyKnown = (event: RunEvent) => {
      if (event.type === "output") {
        const found = runsRef.current[event.runId];
        if (found !== undefined)
          runsRef.current = {
            ...runsRef.current,
            [event.runId]: { ...found, lastLine: event.line },
          };
        setRuns((current) => {
          const existing = current[event.runId];
          if (existing === undefined) return current;
          return {
            ...current,
            [event.runId]: { ...existing, lastLine: event.line },
          };
        });
      } else if (event.type === "error") {
        setError(event.message);
      } else if (event.type === "finished") {
        removeRun(event.runId);
      }
    };

    const reconcileUnknown = () => {
      if (reconcilingRef.current) {
        reconcileDirtyRef.current = true;
        return;
      }
      reconcilingRef.current = true;
      void (async () => {
        do {
          reconcileDirtyRef.current = false;
          try {
            const snapshot = await listRuns();
            if (!live) return;
            // Runs the backend already settled need no buffered replay.
            // Their dead markers stay: a pending start must still see them.
            for (const run of snapshot) {
              if (run.finishedAt !== null) pendingRef.current.delete(run.id);
            }
            const adds = snapshot.filter(
              (run) =>
                run.finishedAt === null &&
                !(run.id in runsRef.current) &&
                !deadRef.current.has(run.id),
            );
            if (adds.length > 0) {
              const buffers = new Map(pendingRef.current);
              const fresh: Record<string, ActiveRun> = {};
              for (const run of adds) fresh[run.id] = toActiveRun(run);
              for (const run of adds) {
                for (const buffered of buffers.get(run.id) ?? []) {
                  if (buffered.type === "output") {
                    const found = fresh[run.id];
                    if (found !== undefined)
                      fresh[run.id] = { ...found, lastLine: buffered.line };
                  } else if (buffered.type === "finished") {
                    delete fresh[run.id];
                  }
                }
              }
              runsRef.current = { ...runsRef.current, ...fresh };
              for (const run of adds) {
                if (!deadRef.current.has(run.id))
                  pendingRef.current.delete(run.id);
              }
              if (Object.keys(fresh).length > 0) {
                setRuns((current) => {
                  const next = { ...current };
                  for (const [runId, run] of Object.entries(fresh)) {
                    if (!(runId in next)) next[runId] = run;
                  }
                  return next;
                });
              }
            }
          } catch {
            // A failed reconcile leaves tracking live; the next event retries.
          }
        } while (reconcileDirtyRef.current && live);
        reconcilingRef.current = false;
      })();
    };

    const routeEvent = (event: RunEvent) => {
      if (event.runId in runsRef.current) {
        applyKnown(event);
        return;
      }
      // Unknown run: a scheduler start, or a start whose `runNext` has not
      // resolved yet. Buffer lifecycle events in order, then reconcile so
      // scheduler-started runs appear in Now Playing.
      if (isLifecycle(event)) {
        const list = pendingRef.current.get(event.runId) ?? [];
        list.push(event);
        pendingRef.current.set(event.runId, list);
      }
      if (event.type === "error") setError(event.message);
      if (event.type === "finished") deadRef.current.add(event.runId);
      reconcileUnknown();
    };

    const handleEvent = (event: RunEvent) => {
      if (hydratingFlag) {
        hydrating.push(event);
        return;
      }
      routeEvent(event);
    };

    void (async () => {
      try {
        const stop = await listenRunEvent(handleEvent);
        if (!live) {
          stop();
          return;
        }
        unlisten = stop;
      } catch (caught) {
        // Without the channel no `finished` can ever clear a run, so runs
        // started here would stick forever. Report it and keep starts
        // blocked until tracking is ready.
        if (live) setError(message(caught));
        return;
      }
      try {
        const snapshot = await listRuns();
        if (!live) return;
        const fresh: Record<string, ActiveRun> = {};
        for (const run of snapshot) {
          if (run.finishedAt === null) fresh[run.id] = toActiveRun(run);
        }
        let failure: string | null = null;
        for (const event of hydrating) {
          if (event.type === "output") {
            const found = fresh[event.runId];
            if (found !== undefined)
              fresh[event.runId] = { ...found, lastLine: event.line };
          } else if (event.type === "error") {
            failure = event.message;
          } else if (event.type === "finished") {
            delete fresh[event.runId];
            deadRef.current.add(event.runId);
          }
        }
        hydrating.length = 0;
        hydratingFlag = false;
        runsRef.current = { ...runsRef.current, ...fresh };
        if (failure !== null) setError(failure);
        if (Object.keys(fresh).length > 0) {
          setRuns((current) => ({ ...fresh, ...current }));
        }
      } catch (caught) {
        if (live) setError(message(caught));
        hydratingFlag = false;
      }
      if (live) {
        readyRef.current = true;
        setReady(true);
      }
    })();

    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  const start = useCallback(async (engine: EngineId) => {
    if (!readyRef.current) {
      setError("Run tracking is not ready yet");
      return;
    }
    setStarting((current) => ({ ...current, [engine]: true }));
    try {
      const run = await runNext({ engine });
      const buffered = pendingRef.current.get(run.id) ?? [];
      const hadError = buffered.some((event) => event.type === "error");
      if (deadRef.current.has(run.id)) {
        // Finished before `runNext` resolved: never re-add it. Any error
        // already reached the banner when the event arrived. The marker
        // stays so a late event cannot resurrect the run via reconcile.
        pendingRef.current.delete(run.id);
      } else {
        if (!(run.id in runsRef.current)) {
          const active = toActiveRun(run);
          runsRef.current = { ...runsRef.current, [run.id]: active };
          setRuns((current) => {
            if (run.id in current) return current;
            return { ...current, [run.id]: active };
          });
        }
        // Replay what arrived early, in order: latest output line wins, a
        // buffered finish wins over the fresh insert.
        const lines = buffered.filter((event) => event.type === "output");
        const last = lines[lines.length - 1];
        if (last !== undefined && last.type === "output") {
          const line = last.line;
          const found = runsRef.current[run.id];
          if (found !== undefined)
            runsRef.current = {
              ...runsRef.current,
              [run.id]: { ...found, lastLine: line },
            };
          setRuns((current) => {
            const existing = current[run.id];
            if (existing === undefined) return current;
            return { ...current, [run.id]: { ...existing, lastLine: line } };
          });
        }
        const failure = [...buffered]
          .reverse()
          .find((event) => event.type === "error");
        if (failure !== undefined && failure.type === "error")
          setError(failure.message);
        if (buffered.some((event) => event.type === "finished")) {
          deadRef.current.add(run.id);
          const kept = { ...runsRef.current };
          delete kept[run.id];
          runsRef.current = kept;
          setRuns((current) => {
            if (!(run.id in current)) return current;
            const next = { ...current };
            delete next[run.id];
            return next;
          });
        }
        if (!deadRef.current.has(run.id)) pendingRef.current.delete(run.id);
      }
      if (!hadError) setError(null);
    } catch (caught) {
      setError(message(caught));
    } finally {
      setStarting((current) => ({ ...current, [engine]: false }));
    }
  }, []);

  const stop = useCallback(async (runId: string) => {
    if (stoppingRef.current.has(runId)) return;
    stoppingRef.current.add(runId);
    setStopping((current) => ({ ...current, [runId]: true }));
    try {
      await stopRun({ runId });
      setError(null);
      // `stopping` stays true until `finished` clears the run alongside it.
    } catch (caught) {
      stoppingRef.current.delete(runId);
      setStopping((current) => {
        const next = { ...current };
        delete next[runId];
        return next;
      });
      setError(message(caught));
    }
  }, []);

  return { runs, starting, stopping, error, ready, start, stop };
}
