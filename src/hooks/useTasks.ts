import { useCallback, useEffect, useState } from "react";
import type { EngineChoice, Task } from "../types";
import {
  addTask,
  deleteTask,
  listTasks,
  listenTaskUpdate,
  updateTask,
} from "../types/ipc";
import { isAbsolute } from "../lib/folder";
import { message } from "../lib/errors";

/**
 * The task list, owned by the store. `tasks === null` means the first load has
 * not finished; an empty array is a real empty queue, and the two must not
 * render the same way.
 *
 * Every command can reject, and the contract returns errors as strings. They
 * surface in `error` rather than being swallowed — "errors reach the UI, no
 * silent failures".
 */
export function useTasks() {
  const [tasks, setTasks] = useState<Task[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;
    // Updates arriving while the snapshot loads are buffered, then replayed
    // by task ID in arrival order. `updatedAt` has only second precision,
    // so it cannot order a claim and its completion within one tick.
    let snapshotReady = false;
    const buffered: Task[] = [];
    const applyUpdate = (list: Task[], updated: Task): Task[] => {
      if (!list.some((t) => t.id === updated.id)) return [...list, updated];
      return list.map((t) => (t.id === updated.id ? updated : t));
    };
    void (async () => {
      try {
        const stop = await listenTaskUpdate((updated) => {
          if (!snapshotReady) buffered.push(updated);
          else setTasks((current) => applyUpdate(current ?? [], updated));
        });
        if (live) unlisten = stop;
        else {
          stop();
          return;
        }
      } catch {
        // No channel means no live queue; the initial read still stands.
      }
      try {
        const list = await listTasks();
        if (!live) return;
        let next = [...list];
        for (const updated of buffered) next = applyUpdate(next, updated);
        buffered.length = 0;
        snapshotReady = true;
        setTasks(next);
      } catch (caught) {
        if (live) {
          setError(message(caught));
          // An empty array, not null: the load finished, it just failed.
          // Buffered updates still apply on top of it.
          let next: Task[] = [];
          for (const updated of buffered) next = applyUpdate(next, updated);
          buffered.length = 0;
          snapshotReady = true;
          setTasks(next);
        }
      }
    })();
    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  const add = useCallback(async (prompt: string, folder: string) => {
    // Last line of defence: the button is disabled without a folder, but the
    // store must never be handed a relative path whatever the UI does.
    if (!isAbsolute(folder)) {
      setError("Task folder must be an absolute path");
      return;
    }
    try {
      const created = await addTask({
        prompt,
        folder,
        size: "m",
        engine: { type: "auto" },
      });
      setTasks((current) => [...(current ?? []), created]);
      setError(null);
    } catch (caught) {
      setError(message(caught));
    }
  }, []);

  const setEngine = useCallback(async (id: string, engine: EngineChoice) => {
    try {
      const updated = await updateTask({ id, engine });
      setTasks((current) =>
        (current ?? []).map((task) => (task.id === id ? updated : task)),
      );
      setError(null);
    } catch (caught) {
      setError(message(caught));
    }
  }, []);

  const remove = useCallback(async (id: string) => {
    try {
      await deleteTask({ id });
      setTasks((current) => (current ?? []).filter((task) => task.id !== id));
      setError(null);
    } catch (caught) {
      setError(message(caught));
    }
  }, []);

  return { tasks, error, add, setEngine, remove };
}
