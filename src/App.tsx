import { useEffect, useMemo, useState } from "react";
import type { EngineChoice, EngineId, LimitWindowKind } from "./types";
import { MOCK_METERS, MOCK_TASKS } from "./mocks";
import { groupMeters, levelFor, usedPct } from "./lib/meters";
import { SCREEN_HEADING, type Screen } from "./lib/screens";
import {
  loadPreferences,
  savePreferences,
  type Accent,
  type Theme,
} from "./lib/preferences";
import { applyAlwaysOnTop } from "./lib/window";
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

  // Engine picked per task. Local only: update_task has no handler yet, so
  // this is the UI half of a call the runner PR will make real.
  const [engineFor, setEngineFor] = useState<Record<string, EngineChoice>>({});
  const [preferences, setPreferences] = useState(loadPreferences);

  // Theme and accent are stamped on the root element so the token blocks in
  // index.css can key off them, and remembered across launches.
  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-widget-theme", preferences.theme);
    root.setAttribute("data-accent", preferences.accent);
    savePreferences(preferences);
  }, [preferences]);

  // Always on top is a real window call, applied on load as well as on change
  // so the setting survives a restart.
  useEffect(() => {
    void applyAlwaysOnTop(preferences.alwaysOnTop);
  }, [preferences.alwaysOnTop]);

  const groups = useMemo(() => groupMeters(MOCK_METERS), []);

  // The reset countdowns are relative to now, so the clock has to advance on
  // its own — otherwise a row reads "resets in 2h 14m" until some unrelated
  // interaction happens to re-render it. A minute is the display granularity.
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(timer);
  }, []);

  // Finished, failed and discarded tasks are history, not queue. The header
  // count and the queue screen read from the same list so they cannot disagree.
  const active = useMemo(
    () =>
      MOCK_TASKS.filter(
        (task) => task.status === "queued" || task.status === "running",
      ),
    [],
  );
  const queue = active.map((task) =>
    engineFor[task.id] ? { ...task, engine: engineFor[task.id] } : task,
  );
  const queued = active.length;

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

  const selectWindow = (engine: EngineId, kind: LimitWindowKind) =>
    setSelected((current) => ({ ...current, [engine]: kind }));

  const toggleRun = (engine: EngineId) =>
    setRunning((current) => ({ ...current, [engine]: !current[engine] }));

  return (
    <main className="widget">
      <TitleStrip
        title={SCREEN_HEADING[screen]}
        status={status}
        screen={screen}
        onOpen={setScreen}
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
        {screen === "tasks" && (
          <Tasks
            tasks={queue}
            onEngine={(id, engine) =>
              setEngineFor((current) => ({ ...current, [id]: engine }))
            }
          />
        )}
        {screen === "settings" && (
          <Settings
            preferences={preferences}
            onTheme={(theme: Theme) =>
              setPreferences((current) => ({ ...current, theme }))
            }
            onAccent={(accent: Accent) =>
              setPreferences((current) => ({ ...current, accent }))
            }
            onToggleAlwaysOnTop={() =>
              setPreferences((current) => ({
                ...current,
                alwaysOnTop: !current.alwaysOnTop,
              }))
            }
          />
        )}
      </div>
    </main>
  );
}

export default App;
