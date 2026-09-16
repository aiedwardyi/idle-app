import type { LimitWindowKind, EngineId, Task } from "../types";
import type { ActiveRun } from "../hooks/useRuns";
import { MeterRow } from "../components/MeterRow";
import { NowPlaying } from "../components/NowPlaying";
import type { EngineMeters } from "../lib/meters";

type Props = {
  groups: EngineMeters[];
  selected: Partial<Record<EngineId, LimitWindowKind>>;
  running: Partial<Record<EngineId, boolean>>;
  busy: Partial<Record<EngineId, boolean>>;
  now: Date;
  nowPlaying: ActiveRun[];
  tasks: Task[] | null;
  stopping: Record<string, boolean>;
  onSelectWindow: (engine: EngineId, kind: LimitWindowKind) => void;
  onToggleRun: (engine: EngineId) => void;
  onStopRun: (runId: string) => void;
};

export function Widget({
  groups,
  selected,
  running,
  busy,
  now,
  nowPlaying,
  tasks,
  stopping,
  onSelectWindow,
  onToggleRun,
  onStopRun,
}: Props) {
  return (
    <div className="meters">
      <NowPlaying
        runs={nowPlaying}
        tasks={tasks}
        now={now}
        stopping={stopping}
        onStop={onStopRun}
      />
      {groups.map((group) => (
        <MeterRow
          key={group.engine}
          group={group}
          selected={selected[group.engine] ?? group.windows[0].window}
          running={running[group.engine] ?? false}
          busy={busy[group.engine] ?? false}
          now={now}
          onSelectWindow={(kind) => onSelectWindow(group.engine, kind)}
          onToggleRun={() => onToggleRun(group.engine)}
        />
      ))}
    </div>
  );
}
