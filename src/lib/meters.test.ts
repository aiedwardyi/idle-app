import { describe, expect, test } from "vitest";
import type { MeterState } from "../types";
import {
  formatTokens,
  formatUntil,
  groupMeters,
  levelFor,
  totalUsage,
  usedPct,
} from "./meters";

const meter = (over: Partial<MeterState> = {}): MeterState => ({
  engine: "claude",
  window: "fiveHour",
  used: { input: 100, output: 50, cache: 25 },
  capacityEst: 1000,
  calibrated: true,
  remainingPct: null,
  resetsAt: null,
  ...over,
});

describe("totalUsage", () => {
  test("sums the three counters", () => {
    expect(totalUsage({ input: 100, output: 50, cache: 25 })).toBe(175);
  });
});

describe("usedPct", () => {
  test("prefers remainingPct when the backend sends it", () => {
    expect(usedPct(meter({ remainingPct: 26.4 }))).toBeCloseTo(73.6);
  });

  test("falls back to used over capacityEst", () => {
    expect(usedPct(meter())).toBeCloseTo(17.5);
  });

  test("returns null when neither is known", () => {
    expect(usedPct(meter({ capacityEst: null }))).toBeNull();
  });

  test("returns null rather than dividing by zero", () => {
    expect(usedPct(meter({ capacityEst: 0 }))).toBeNull();
  });

  test("never goes below zero", () => {
    expect(usedPct(meter({ remainingPct: 140 }))).toBe(0);
  });

  test("reports over-limit rather than clamping to 100", () => {
    expect(usedPct(meter({ remainingPct: -22 }))).toBeCloseTo(122);
  });
});

describe("levelFor", () => {
  test.each([
    [null, "unknown"],
    [0, "ok"],
    [69.9, "ok"],
    [70, "tight"],
    [87.9, "tight"],
    [88, "near"],
    [99.9, "near"],
    [100, "hit"],
    [140, "hit"],
  ])("%s -> %s", (pct, expected) => {
    expect(levelFor(pct)).toBe(expected);
  });
});

describe("formatTokens", () => {
  test.each([
    [175, "175"],
    [1_500, "2k"],
    [906_000, "906k"],
    [1_776_000, "1.8M"],
    [41_000_000, "41M"],
  ])("%s -> %s", (n, expected) => {
    expect(formatTokens(n)).toBe(expected);
  });
});

describe("formatUntil", () => {
  const now = new Date("2026-09-01T12:00:00Z");

  test("null when there is no reset time", () => {
    expect(formatUntil(null, now)).toBeNull();
  });

  test("null on an unparseable timestamp", () => {
    expect(formatUntil("not-a-date", now)).toBeNull();
  });

  test("minutes under an hour", () => {
    expect(formatUntil("2026-09-01T12:41:00Z", now)).toBe("41m");
  });

  test("pads the minutes so rows stay aligned", () => {
    expect(formatUntil("2026-09-01T14:06:00Z", now)).toBe("2h 06m");
  });

  test("days past 24 hours", () => {
    expect(formatUntil("2026-09-05T13:00:00Z", now)).toBe("4d 1h");
  });

  test("a reset time in the past reads as now", () => {
    expect(formatUntil("2026-09-01T11:00:00Z", now)).toBe("now");
  });
});

describe("groupMeters", () => {
  const meters: MeterState[] = [
    meter({ engine: "grok", window: "weekly" }),
    meter({ engine: "claude", window: "weekly" }),
    meter({ engine: "claude", window: "fiveHour" }),
    meter({ engine: "antigravity", window: "daily" }),
  ];

  test("one entry per engine, in display order", () => {
    expect(groupMeters(meters).map((g) => g.engine)).toEqual([
      "claude",
      "antigravity",
      "grok",
    ]);
  });

  test("windows are shortest first", () => {
    const claude = groupMeters(meters)[0];
    expect(claude.windows.map((w) => w.window)).toEqual(["fiveHour", "weekly"]);
  });

  test("engines with no meters are omitted", () => {
    expect(groupMeters([]).length).toBe(0);
  });

  test("does not mutate the input order", () => {
    const input = [...meters];
    groupMeters(input);
    expect(input[0].engine).toBe("grok");
  });
});
