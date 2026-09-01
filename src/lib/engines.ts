import type { EngineId, LimitWindowKind } from "../types";

/** Display order on the widget. Fixed, so rows never reshuffle between renders. */
export const ENGINE_ORDER: readonly EngineId[] = [
  "claude",
  "codex",
  "antigravity",
  "grok",
];

export const ENGINE_LABEL: Record<EngineId, string> = {
  claude: "Claude",
  codex: "Codex",
  antigravity: "Antigravity",
  grok: "Grok",
};

/** Shortest window first — the one that runs out soonest is the one you care about. */
const WINDOW_RANK: Record<LimitWindowKind, number> = {
  fiveHour: 0,
  daily: 1,
  weekly: 2,
};

export const WINDOW_LABEL: Record<LimitWindowKind, string> = {
  fiveHour: "5h",
  daily: "24h",
  weekly: "7d",
};

export function windowRank(kind: LimitWindowKind): number {
  return WINDOW_RANK[kind];
}
