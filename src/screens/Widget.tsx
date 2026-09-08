import type { LimitWindowKind, EngineId } from "../types";
import { MeterRow } from "../components/MeterRow";
import type { EngineMeters } from "../lib/meters";

type Props = {
  groups: EngineMeters[];
  selected: Partial<Record<EngineId, LimitWindowKind>>;
  running: Partial<Record<EngineId, boolean>>;
  now: Date;
  levels: { tight: number; near: number };
  showFooter: boolean;
  /** The window each engine opens on, until the user picks another. */
  defaultWindow: Partial<Record<EngineId, LimitWindowKind>>;
  onSelectWindow: (engine: EngineId, kind: LimitWindowKind) => void;
  onToggleRun: (engine: EngineId) => void;
};

export function Widget({
  groups,
  selected,
  running,
  now,
  levels,
  showFooter,
  defaultWindow,
  onSelectWindow,
  onToggleRun,
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
          running={running[group.engine] ?? false}
          now={now}
          levels={levels}
          showFooter={showFooter}
          onSelectWindow={(kind) => onSelectWindow(group.engine, kind)}
          onToggleRun={() => onToggleRun(group.engine)}
        />
      ))}
    </div>
  );
}
