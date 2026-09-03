import { describe, expect, test } from "vitest";
import type { Task } from "../types";
import { sortTasks } from "./sort";
import type { Priority } from "./priority";

const task = (id: string, engine: Task["engine"]): Task => ({
  id,
  prompt: id,
  folder: "/w",
  size: "m",
  engine,
  status: "queued",
  createdAt: "2026-09-01T00:00:00Z",
  updatedAt: "2026-09-01T00:00:00Z",
});

const TASKS: Task[] = [
  task("a", { type: "auto" }),
  task("b", { type: "fixed", engine: "grok" }),
  task("c", { type: "fixed", engine: "claude" }),
  task("d", { type: "auto" }),
];

const ids = (list: Task[]) => list.map((t) => t.id);

describe("sortTasks", () => {
  test("added keeps the incoming order", () => {
    expect(ids(sortTasks(TASKS, "added", {}))).toEqual(["a", "b", "c", "d"]);
  });

  test("priority puts high first and low last", () => {
    const priorities: Record<string, Priority> = { b: "low", c: "high" };
    expect(ids(sortTasks(TASKS, "priority", priorities))).toEqual([
      "c",
      "a",
      "d",
      "b",
    ]);
  });

  test("engine groups in display order with auto last", () => {
    expect(ids(sortTasks(TASKS, "engine", {}))).toEqual(["c", "b", "a", "d"]);
  });

  test("ties keep their original order in every mode", () => {
    // a and d are both auto and both default priority
    expect(ids(sortTasks(TASKS, "priority", {})).slice(0, 4)).toEqual([
      "a",
      "b",
      "c",
      "d",
    ]);
    const engineSorted = ids(sortTasks(TASKS, "engine", {}));
    expect(engineSorted.indexOf("a")).toBeLessThan(engineSorted.indexOf("d"));
  });

  test("does not mutate the input", () => {
    const input = [...TASKS];
    sortTasks(input, "priority", { b: "high" });
    expect(ids(input)).toEqual(["a", "b", "c", "d"]);
  });

  test("an unknown priority falls back to normal rather than dropping the task", () => {
    const sorted = sortTasks(TASKS, "priority", {
      a: "nonsense" as Priority,
    });
    expect(sorted).toHaveLength(4);
  });
});
