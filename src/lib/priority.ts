/**
 * Priority is NOT in the contract: Task has no such field, the tasks table has
 * no column, and update_task cannot carry it. Until that changes it lives here
 * and in localStorage, which means the runner cannot see it — the picker sets
 * an intention, not a schedule.
 *
 * To make it real, CONTRACT.md needs:
 *   TaskPriority  "low" | "normal" | "high"
 *   Task          + priority: TaskPriority
 *   UpdateTask    + priority?: TaskPriority
 *   tasks table   + priority TEXT NOT NULL DEFAULT 'normal', index on it
 * That is a src-tauri change, and src-tauri belongs to @aiedwardyi.
 */
export const PRIORITIES = ["low", "normal", "high"] as const;
export type Priority = (typeof PRIORITIES)[number];

export const PRIORITY_LABEL: Record<Priority, string> = {
  low: "Low",
  normal: "Normal",
  high: "High",
};

/** Ordinal, so it is encoded by how many bars are lit, not by hue. */
export const PRIORITY_BARS: Record<Priority, number> = {
  low: 1,
  normal: 2,
  high: 3,
};

export const DEFAULT_PRIORITY: Priority = "normal";

const KEY = "idle.priorities";

export function loadPriorities(): Record<string, Priority> {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (raw === null) return {};
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return {};
    const out: Record<string, Priority> = {};
    for (const [id, value] of Object.entries(
      parsed as Record<string, unknown>,
    )) {
      if (PRIORITIES.includes(value as Priority)) out[id] = value as Priority;
    }
    return out;
  } catch {
    return {};
  }
}

export function savePriorities(map: Record<string, Priority>): void {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(map));
  } catch {
    // Losing a priority is better than losing the widget.
  }
}
