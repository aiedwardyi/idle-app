import { useCallback, useEffect, useState } from "react";
import type { EngineChoice, Task } from "../types";
import { addTask, deleteTask, listTasks, updateTask } from "../types/ipc";
import { isAbsolute } from "../lib/folder";

const message = (error: unknown): string =>
  typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : "Unknown error";

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
    void (async () => {
      try {
        const list = await listTasks();
        if (live) setTasks(list);
      } catch (caught) {
        if (live) {
          setError(message(caught));
          // An empty array, not null: the load finished, it just failed.
          setTasks([]);
        }
      }
    })();
    return () => {
      live = false;
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
