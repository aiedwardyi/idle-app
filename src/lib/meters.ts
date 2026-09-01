import type { EngineId, LimitWindowKind, MeterState, Usage } from "../types";
import { ENGINE_ORDER, windowRank } from "./engines";

export type MeterLevel = "unknown" | "ok" | "tight" | "near" | "hit";

export const LEVEL_WORD: Record<MeterLevel, string> = {
  unknown: "no estimate",
  ok: "ok",
  tight: "tight",
  near: "near limit",
  hit: "limit hit",
};

export function totalUsage(used: Usage): number {
  return used.input + used.output + used.cache;
}

/**
 * Percentage of the window consumed. `remainingPct` is authoritative when the
 * backend sends it; otherwise fall back to used/capacityEst. Returns null when
 * neither is known — the UI must not invent a number.
 */
export function usedPct(meter: MeterState): number | null {
  if (meter.remainingPct !== null) {
    return Math.max(0, 100 - meter.remainingPct);
  }
  if (meter.capacityEst !== null && meter.capacityEst > 0) {
    return Math.max(0, (totalUsage(meter.used) / meter.capacityEst) * 100);
  }
  return null;
}

export function levelFor(pct: number | null): MeterLevel {
  if (pct === null) return "unknown";
  if (pct >= 100) return "hit";
  if (pct >= 88) return "near";
  if (pct >= 70) return "tight";
  return "ok";
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) {
    return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 0 : 1)}M`;
  }
  if (n >= 1_000) return `${Math.round(n / 1_000)}k`;
  return String(n);
}

/** "2h 14m", "4d 1h", "now". Null when the backend has no reset time yet. */
export function formatUntil(resetsAt: string | null, now: Date): string | null {
  if (resetsAt === null) return null;
  const target = Date.parse(resetsAt);
  if (Number.isNaN(target)) return null;

  const minutes = Math.round((target - now.getTime()) / 60_000);
  if (minutes <= 0) return "now";
  if (minutes < 60) return `${minutes}m`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ${String(minutes % 60).padStart(2, "0")}m`;
  return `${Math.floor(hours / 24)}d ${hours % 24}h`;
}

export type EngineMeters = { engine: EngineId; windows: MeterState[] };

/**
 * One entry per engine, windows shortest-first. Derived from whatever the
 * backend sends rather than a hardcoded table, so the UI follows the contract's
 * default_windows() without duplicating it.
 */
export function groupMeters(meters: MeterState[]): EngineMeters[] {
  const byEngine = new Map<EngineId, MeterState[]>();
  for (const meter of meters) {
    const bucket = byEngine.get(meter.engine);
    if (bucket) bucket.push(meter);
    else byEngine.set(meter.engine, [meter]);
  }

  return ENGINE_ORDER.filter((engine) => byEngine.has(engine)).map(
    (engine) => ({
      engine,
      windows: [...(byEngine.get(engine) ?? [])].sort(
        (a, b) => windowRank(a.window) - windowRank(b.window),
      ),
    }),
  );
}

export function defaultWindow(windows: MeterState[]): LimitWindowKind | null {
  return windows.length > 0 ? windows[0].window : null;
}
