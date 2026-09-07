import type { EngineStatus, MeterState, Task } from "../types";

/**
 * Stand-in for the Tauri command layer. Tests drive the same nine commands the
 * app calls, so a change to an argument name breaks a test rather than only
 * the running app.
 */
type Listener = (payload: unknown) => void;

export const ipc = {
  tasks: [] as Task[],
  meters: [] as MeterState[],
  engines: [] as EngineStatus[],
  /** Command name -> error string, to make one command reject. */
  fail: {} as Record<string, string>,
  calls: [] as { cmd: string; args: Record<string, unknown> }[],
  listeners: {} as Record<string, Listener[]>,
  /** Monotonic: array length reuses an id after a delete. */
  nextId: 0,
};

const task = (id: string, prompt: string, over: Partial<Task> = {}): Task => ({
  id,
  prompt,
  folder: "/Users/you/code/ledger",
  size: "m",
  engine: { type: "auto" },
  status: "queued",
  createdAt: "2026-09-01T06:00:00Z",
  updatedAt: "2026-09-01T06:00:00Z",
  ...over,
});

const meter = (
  engine: MeterState["engine"],
  window: MeterState["window"],
  over: Partial<MeterState> = {},
): MeterState => ({
  engine,
  window,
  used: { input: 1_000_000, output: 200_000, cache: 40_000 },
  capacityEst: 2_400_000,
  calibrated: true,
  remainingPct: 26.4,
  resetsAt: new Date(Date.now() + 8_040_000).toISOString(),
  ...over,
});

export function resetIpc(): void {
  ipc.tasks = [
    task("t1", "Add retry to the sync worker", { status: "running" }),
    task("t2", "Write tests for the CSV parser", {
      size: "s",
      engine: { type: "fixed", engine: "claude" },
    }),
    task("t3", "Draft the migration plan for v3", { size: "l" }),
    task("t4", "Fix flaky snapshot on Windows CI", { status: "done" }),
  ];
  ipc.meters = [
    meter("claude", "fiveHour"),
    meter("claude", "weekly", { remainingPct: 44.6 }),
    meter("codex", "fiveHour", { remainingPct: 49.7 }),
    meter("codex", "weekly", { remainingPct: 58.3 }),
    meter("antigravity", "daily", { calibrated: false, remainingPct: null }),
    meter("grok", "weekly", { remainingPct: 77.4 }),
  ];
  ipc.engines = [];
  ipc.fail = {};
  ipc.calls = [];
  ipc.listeners = {};
  ipc.nextId = ipc.tasks.length;
}

export function emit(event: string, payload: unknown): void {
  for (const listener of ipc.listeners[event] ?? []) listener(payload);
}

export async function handleInvoke(
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<unknown> {
  ipc.calls.push({ cmd, args });
  if (ipc.fail[cmd] !== undefined) throw ipc.fail[cmd];

  switch (cmd) {
    case "list_tasks":
      return [...ipc.tasks];
    case "add_task": {
      ipc.nextId += 1;
      const created = task(`t${ipc.nextId}`, args.prompt as string, {
        folder: args.folder as string,
        size: args.size as Task["size"],
        engine: args.engine as Task["engine"],
      });
      ipc.tasks = [...ipc.tasks, created];
      return created;
    }
    case "update_task": {
      const found = ipc.tasks.find((t) => t.id === args.id);
      if (found === undefined) throw "no such task";
      const updated = { ...found, ...args } as Task;
      ipc.tasks = ipc.tasks.map((t) => (t.id === updated.id ? updated : t));
      return updated;
    }
    case "delete_task":
      ipc.tasks = ipc.tasks.filter((t) => t.id !== args.id);
      return null;
    case "get_meters":
      return [...ipc.meters];
    case "get_engines":
      return [...ipc.engines];
    case "list_runs":
      return [];
    case "stop_run":
      return null;
    default:
      throw `unmocked command: ${cmd}`;
  }
}

export function addListener(event: string, listener: Listener): () => void {
  ipc.listeners[event] = [...(ipc.listeners[event] ?? []), listener];
  return () => {
    ipc.listeners[event] = (ipc.listeners[event] ?? []).filter(
      (l) => l !== listener,
    );
  };
}
