import type { MeterState, Task } from "./types";

/**
 * Static stand-ins for what get_meters / list_tasks will return. PR-03 is the
 * shell only: no IPC handlers exist yet (see src-tauri/src/ipc.rs), so the
 * screens are typed against the contract and fed from here. Delete this file in
 * the PR that wires the real commands up.
 */

const inHours = (hours: number): string =>
  new Date(Date.now() + hours * 3_600_000).toISOString();

export const MOCK_METERS: MeterState[] = [
  {
    engine: "claude",
    window: "fiveHour",
    used: { input: 1_240_000, output: 402_000, cache: 134_000 },
    capacityEst: 2_400_000,
    calibrated: true,
    remainingPct: 26.4,
    resetsAt: inHours(2.23),
  },
  {
    engine: "claude",
    window: "weekly",
    used: { input: 16_800_000, output: 4_100_000, cache: 1_600_000 },
    capacityEst: 41_000_000,
    calibrated: true,
    remainingPct: 44.6,
    resetsAt: inHours(96.5),
  },
  {
    engine: "codex",
    window: "fiveHour",
    used: { input: 640_000, output: 210_000, cache: 56_000 },
    capacityEst: 1_800_000,
    calibrated: true,
    remainingPct: 49.7,
    resetsAt: inHours(4.1),
  },
  {
    engine: "codex",
    window: "weekly",
    used: { input: 7_900_000, output: 2_050_000, cache: 880_000 },
    capacityEst: 26_000_000,
    calibrated: true,
    remainingPct: 58.3,
    resetsAt: inHours(121),
  },
  {
    // Uncalibrated: capacityEst is a guess, so the row must not read as fact.
    engine: "antigravity",
    window: "daily",
    used: { input: 3_600_000, output: 1_020_000, cache: 320_000 },
    capacityEst: 5_200_000,
    calibrated: false,
    remainingPct: null,
    resetsAt: inHours(9.75),
  },
  {
    engine: "grok",
    window: "weekly",
    used: { input: 2_100_000, output: 620_000, cache: 90_000 },
    capacityEst: 12_000_000,
    calibrated: true,
    remainingPct: 77.4,
    resetsAt: inHours(58.3),
  },
];

export const MOCK_TASKS: Task[] = [
  {
    id: "6f1c1d0e-0000-4000-8000-000000000001",
    prompt: "Add retry to the sync worker",
    folder: "/Users/you/code/ledger",
    size: "m",
    engine: { type: "auto" },
    status: "running",
    createdAt: "2026-09-01T06:10:00Z",
    updatedAt: "2026-09-01T07:02:00Z",
  },
  {
    id: "6f1c1d0e-0000-4000-8000-000000000002",
    prompt: "Write tests for the CSV parser",
    folder: "/Users/you/code/ledger",
    size: "s",
    engine: { type: "fixed", engine: "claude" },
    status: "queued",
    createdAt: "2026-09-01T06:12:00Z",
    updatedAt: "2026-09-01T06:12:00Z",
  },
  {
    id: "6f1c1d0e-0000-4000-8000-000000000003",
    prompt: "Draft the migration plan for v3",
    folder: "/Users/you/notes",
    size: "l",
    engine: { type: "auto" },
    status: "queued",
    createdAt: "2026-09-01T06:20:00Z",
    updatedAt: "2026-09-01T06:20:00Z",
  },
  {
    id: "6f1c1d0e-0000-4000-8000-000000000004",
    prompt: "Fix flaky snapshot on Windows CI",
    folder: "/Users/you/code/idle-app",
    size: "s",
    engine: { type: "fixed", engine: "codex" },
    status: "done",
    createdAt: "2026-08-31T18:40:00Z",
    updatedAt: "2026-08-31T19:05:00Z",
  },
];
