import type { LimitWindowKind, EngineId } from "../types";
import { MeterRow } from "../components/MeterRow";
import type { EngineMeters } from "../lib/meters";

type Props = {
  groups: EngineMeters[];
  selected: Partial<Record<EngineId, LimitWindowKind>>;
  running: Partial<Record<EngineId, boolean>>;
  now: Date;
  onSelectWindow: (engine: EngineId, kind: LimitWindowKind) => void;
  onToggleRun: (engine: EngineId) => void;
};

export function Widget({
  groups,
  selected,
  running,
  now,
  onSelectWindow,
  onToggleRun,
}: Props) {
  return (
    <div className="meters">
      {groups.map((group) => (
        <MeterRow
          key={group.engine}
          group={group}
          selected={selected[group.engine] ?? group.windows[0].window}
          running={running[group.engine] ?? false}
          now={now}
          onSelectWindow={(kind) => onSelectWindow(group.engine, kind)}
          onToggleRun={() => onToggleRun(group.engine)}
        />
      ))}
    </div>
  );
}
