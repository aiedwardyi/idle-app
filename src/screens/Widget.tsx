import type { LimitWindowKind, EngineId } from "../types";
import { MeterRow } from "../components/MeterRow";
import type { EngineMeters } from "../lib/meters";

type Props = {
  groups: EngineMeters[];
  selected: Partial<Record<EngineId, LimitWindowKind>>;
  /** Run id per engine, for the ones with something in flight. */
  runs: Partial<Record<EngineId, string>>;
  now: Date;
  levels: { tight: number; near: number };
  showFooter: boolean;
  /** The window each engine opens on, until the user picks another. */
  defaultWindow: Partial<Record<EngineId, LimitWindowKind>>;
  onSelectWindow: (engine: EngineId, kind: LimitWindowKind) => void;
  onStop: (runId: string) => void;
};

export function Widget({
  groups,
  selected,
  runs,
  now,
  levels,
  showFooter,
  defaultWindow,
  onSelectWindow,
  onStop,
}: Props) {
  return (
    <div className="meters">
      {groups.map((group) => (
        <MeterRow
          key={group.engine}
          group={group}
          selected={
            selected[group.engine] ??
            defaultWindow[group.engine] ??
            group.windows[0].window
          }
          runId={runs[group.engine] ?? null}
          now={now}
          levels={levels}
          showFooter={showFooter}
          onSelectWindow={(kind) => onSelectWindow(group.engine, kind)}
          onStop={() => {
            const runId = runs[group.engine];
            if (runId !== undefined) onStop(runId);
          }}
        />
      ))}
    </div>
  );
}
