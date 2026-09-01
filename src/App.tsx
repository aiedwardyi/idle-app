import { useMemo, useState } from "react";
import type { EngineId, LimitWindowKind } from "./types";
import { MOCK_METERS, MOCK_TASKS } from "./mocks";
import { groupMeters, levelFor, usedPct } from "./lib/meters";
import { SCREEN_TITLE, type Screen } from "./lib/screens";
import { TitleStrip } from "./components/TitleStrip";
import { Widget } from "./screens/Widget";
import { Tasks } from "./screens/Tasks";
import { Settings } from "./screens/Settings";

function App() {
  const [screen, setScreen] = useState<Screen>("widget");
  const [selected, setSelected] = useState<
    Partial<Record<EngineId, LimitWindowKind>>
  >({});
  const [running, setRunning] = useState<Partial<Record<EngineId, boolean>>>(
    {},
  );
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);

  const groups = useMemo(() => groupMeters(MOCK_METERS), []);
  const now = new Date();

  const queued = MOCK_TASKS.filter(
    (task) => task.status === "queued" || task.status === "running",
  ).length;

  const live = groups.filter((group) => {
    if (!running[group.engine]) return false;
    const window = selected[group.engine] ?? group.windows[0].window;
    const meter =
      group.windows.find((w) => w.window === window) ?? group.windows[0];
    return levelFor(usedPct(meter)) !== "hit";
  }).length;

  const status =
    screen === "widget"
      ? `${queued} queued · ${live === 0 ? "paused" : `${live} ${live === 1 ? "engine" : "engines"} working`}`
      : `${queued} queued`;

  const openScreen = (next: Screen) =>
    setScreen((current) => (current === next ? "widget" : next));

  const selectWindow = (engine: EngineId, kind: LimitWindowKind) =>
    setSelected((current) => ({ ...current, [engine]: kind }));

  const toggleRun = (engine: EngineId) =>
    setRunning((current) => ({ ...current, [engine]: !current[engine] }));

  return (
    <main className="widget">
      <TitleStrip
        title={SCREEN_TITLE[screen]}
        status={status}
        screen={screen}
        onOpen={openScreen}
      />

      <div className="body">
        {screen === "widget" && (
          <Widget
            groups={groups}
            selected={selected}
            running={running}
            now={now}
            onSelectWindow={selectWindow}
            onToggleRun={toggleRun}
          />
        )}
        {screen === "tasks" && <Tasks tasks={MOCK_TASKS} />}
        {screen === "settings" && (
          <Settings
            alwaysOnTop={alwaysOnTop}
            onToggleAlwaysOnTop={() => setAlwaysOnTop((value) => !value)}
          />
        )}
      </div>
    </main>
  );
}

export default App;
