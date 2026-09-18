import { describe, expect, test } from "vitest";
import type { Run } from "../types";
import {
  RESULT_LABEL,
  latestRun,
  resultDuration,
  resultState,
  resultWord,
  runsByTask,
} from "./results";

const run = (over: Partial<Run> = {}): Run => ({
  id: "r1",
  taskId: "t1",
  engine: "claude",
  startedAt: "2026-09-01T01:00:00Z",
  finishedAt: "2026-09-01T03:14:00Z",
  exitReason: "ok",
  usage: { input: 0, output: 0, cache: 0 },
  snapshotId: null,
  ...over,
});

describe("RESULT_LABEL", () => {
  test("covers the contract's exit reasons, not the issue's shorthand", () => {
    expect(RESULT_LABEL).toEqual({
      ok: "done",
      failed: "failed",
      limitHit: "limit hit",
      cancelled: "stopped",
      timeout: "timed out",
    });
  });
});

describe("resultState", () => {
  test("three buckets: still working, clean, wants a person", () => {
    expect(resultState(run())).toBe("done");
    expect(resultState(run({ exitReason: "failed" }))).toBe("issue");
    expect(resultState(run({ exitReason: "timeout" }))).toBe("issue");
    expect(resultState(run({ exitReason: "limitHit" }))).toBe("issue");
    // A stop leaves the job undone just as a failure does, so it is not green.
    expect(resultState(run({ exitReason: "cancelled" }))).toBe("issue");
  });

  test("either end missing means the run has not settled", () => {
    expect(resultState(run({ finishedAt: null, exitReason: null }))).toBe(
      "ongoing",
    );
    expect(resultState(run({ finishedAt: null }))).toBe("ongoing");
    expect(resultState(run({ exitReason: null }))).toBe("ongoing");
  });
});

describe("resultWord", () => {
  test("names the exit reason, or says it is still running", () => {
    expect(resultWord(run())).toBe("done");
    expect(resultWord(run({ exitReason: "limitHit" }))).toBe("limit hit");
    expect(resultWord(run({ finishedAt: null, exitReason: null }))).toBe(
      "running",
    );
    // Settled-looking but unfinished: the state wins over the stale reason.
    expect(resultWord(run({ finishedAt: null }))).toBe("running");
  });
});

describe("resultDuration", () => {
  test("rounds to minutes, hours, days", () => {
    expect(resultDuration(run())).toBe("2h");
    expect(
      resultDuration(
        run({
          startedAt: "2026-09-01T01:00:00Z",
          finishedAt: "2026-09-01T01:41:00Z",
        }),
      ),
    ).toBe("41m");
    expect(
      resultDuration(
        run({
          startedAt: "2026-09-01T01:00:00Z",
          finishedAt: "2026-09-04T03:00:00Z",
        }),
      ),
    ).toBe("3d 2h");
  });

  test("null when either end is missing or unparseable", () => {
    expect(resultDuration(run({ finishedAt: null }))).toBeNull();
    expect(resultDuration(run({ finishedAt: "not-a-date" }))).toBeNull();
  });
});

describe("runsByTask", () => {
  test("keys every run by the task it belongs to", () => {
    const grouped = runsByTask([
      run({ id: "a" }),
      run({ id: "b", taskId: "t2" }),
      run({ id: "c" }),
    ]);
    expect(Object.keys(grouped).sort()).toEqual(["t1", "t2"]);
    expect(grouped.t1.map((r) => r.id)).toEqual(["a", "c"]);
  });

  test("a task with no runs is absent, not empty", () => {
    expect(runsByTask([])).toEqual({});
  });
});

describe("latestRun", () => {
  test("live work wins, because it is the only part still changing", () => {
    const live = run({ id: "live", finishedAt: null, exitReason: null });
    const old = run({ id: "old", finishedAt: "2026-09-02T00:00:00Z" });
    expect(latestRun([old, live])?.id).toBe("live");
  });

  test("otherwise the attempt that finished last", () => {
    const first = run({ id: "first", finishedAt: "2026-09-01T02:00:00Z" });
    const second = run({
      id: "second",
      exitReason: "failed",
      finishedAt: "2026-09-01T05:00:00Z",
    });
    expect(latestRun([first, second])?.id).toBe("second");
    expect(latestRun([second, first])?.id).toBe("second");
  });

  test("null when the task has never run", () => {
    expect(latestRun([])).toBeNull();
  });
});
