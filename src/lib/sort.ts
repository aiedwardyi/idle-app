import type { Task } from "../types";
import { ENGINE_ORDER } from "./engines";
import { DEFAULT_PRIORITY, type Priority } from "./priority";

export const SORTS = ["added", "priority", "engine", "manual"] as const;
export type Sort = (typeof SORTS)[number];

export const SORT_LABEL: Record<Sort, string> = {
  added: "Added",
  priority: "Priority",
  engine: "Engine",
  manual: "Manual",
};

const PRIORITY_RANK: Record<Priority, number> = { high: 0, normal: 1, low: 2 };

/** Fixed engines in display order; Auto sorts after them. */
function engineRank(task: Task): number {
  if (task.engine.type === "auto") return ENGINE_ORDER.length;
  const index = ENGINE_ORDER.indexOf(task.engine.engine);
  return index === -1 ? ENGINE_ORDER.length : index;
}

/**
 * Stable in every mode: ties fall back to the order the tasks arrived in, so
 * re-sorting never shuffles rows that compare equal.
 */
export function sortTasks(
  tasks: Task[],
  sort: Sort,
  priorities: Record<string, Priority>,
  order: string[] = [],
): Task[] {
  // Ids the user has never dragged are not in `order`; they sort after the
  // ones that are, in arrival order, so a new task appears at the end.
  const rank = new Map(order.map((id, index) => [id, index]));
  const manual = (task: Task) => rank.get(task.id) ?? order.length;

  const keyed = tasks.map((task, index) => ({ task, index }));

  const compare: Record<Sort, (a: number, b: number) => number> = {
    added: () => 0,
    priority: (a, b) =>
      PRIORITY_RANK[priorities[keyed[a].task.id] ?? DEFAULT_PRIORITY] -
      PRIORITY_RANK[priorities[keyed[b].task.id] ?? DEFAULT_PRIORITY],
    engine: (a, b) => engineRank(keyed[a].task) - engineRank(keyed[b].task),
    manual: (a, b) => manual(keyed[a].task) - manual(keyed[b].task),
  };

  return keyed
    .map((entry, position) => ({ ...entry, position }))
    .sort((a, b) => compare[sort](a.position, b.position) || a.index - b.index)
    .map((entry) => entry.task);
}
