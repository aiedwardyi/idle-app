import type { LimitWindowKind, MeterState } from "../types";
import { ENGINE_LABEL, WINDOW_LABEL } from "../lib/engines";
import {
  LEVEL_WORD,
  formatTokens,
  formatUntil,
  levelFor,
  totalUsage,
  usedPct,
  type EngineMeters,
  type MeterLevel,
} from "../lib/meters";
import { Icon } from "./Icon";

// A clock means "there is still time"; an exhausted meter and an unknown one
// both had it, and both were wrong about it.
const LEVEL_ICON = {
  unknown: "unknown",
  ok: "ok",
  tight: "warn",
  near: "near",
  hit: "blocked",
} as const satisfies Record<MeterLevel, string>;

type Props = {
  group: EngineMeters;
  selected: LimitWindowKind;
  running: boolean;
  now: Date;
  onSelectWindow: (kind: LimitWindowKind) => void;
  onToggleRun: () => void;
};

export function MeterRow({
  group,
  selected,
  running,
  now,
  onSelectWindow,
  onToggleRun,
}: Props) {
  const meter: MeterState =
    group.windows.find((w) => w.window === selected) ?? group.windows[0];
  const label = ENGINE_LABEL[group.engine];
  const pct = usedPct(meter);
  const level = levelFor(pct);
  const exhausted = level === "hit";
  const resets = formatUntil(meter.resetsAt, now);

  return (
    <div className="meter-row" data-level={level}>
      {/* Time to reset is the headline: it is the number you act on. The
          percentage is context and sits in the footer. */}
      <div className="mhead">
        <span className="mname">{label}</span>
        <span className="resets">
          {resets === null ? (
            <b>&mdash;</b>
          ) : exhausted ? (
            <>
              <span className="rlabel">back in</span>
              <b>{resets}</b>
            </>
          ) : (
            <>
              <b>{resets}</b>
              <span className="rlabel">left</span>
            </>
          )}
        </span>
      </div>

      <div className="mbar">
        <button
          type="button"
          className="rowplay"
          data-running={running}
          disabled={exhausted}
          onClick={onToggleRun}
          aria-label={
            exhausted
              ? `${label} has no headroom left`
              : running
                ? `Pause ${label}`
                : `Work the queue with ${label}`
          }
        >
          <Icon name={running ? "pause" : "play"} size={11} />
        </button>

        <div className="track">
          <div
            className="fill"
            data-calibrated={meter.calibrated}
            data-running={running}
            style={{ width: `${Math.min(100, pct ?? 0)}%` }}
          />
        </div>

        {/* Fixed-width slot: keeps every engine's bar the same length whether
            it gets a two-state switch or a single flat chip. */}
        <div className="winslot">
          {group.windows.length > 1 ? (
            <div
              className="rowseg"
              role="group"
              aria-label={`${label} limit window`}
            >
              {group.windows.map((w) => (
                <button
                  key={w.window}
                  type="button"
                  aria-pressed={w.window === meter.window}
                  onClick={() => onSelectWindow(w.window)}
                >
                  {WINDOW_LABEL[w.window]}
                </button>
              ))}
            </div>
          ) : (
            <span className="rowwin-static">{WINDOW_LABEL[meter.window]}</span>
          )}
        </div>
      </div>

      <div className="mfoot">
        <span className="state">
          <Icon name={LEVEL_ICON[level]} size={11} strokeWidth={2.6} />
          {LEVEL_WORD[level]}
        </span>
        <span className="mpct">
          {pct === null
            ? "—"
            : `${meter.calibrated ? "" : "~"}${Math.round(pct)}% used`}
        </span>
        <span className="tokens">
          {formatTokens(totalUsage(meter.used))}
          {meter.capacityEst === null
            ? ""
            : ` / ${meter.calibrated ? "" : "~"}${formatTokens(meter.capacityEst)}`}
        </span>
      </div>
    </div>
  );
}
