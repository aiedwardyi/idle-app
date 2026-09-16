import { useEffect, useMemo, useState } from "react";
import type { EngineId, LimitWindowKind } from "./types";
import { useTasks } from "./hooks/useTasks";
import { useMeters } from "./hooks/useMeters";
import { useRuns } from "./hooks/useRuns";
import { defaultFolder } from "./lib/folder";
import { groupMeters } from "./lib/meters";
import { SCREEN_HEADING, type Screen } from "./lib/screens";
import {
  loadPreferences,
  savePreferences,
  type Accent,
  type Mode,
  type Theme,
} from "./lib/preferences";
import type { Sort } from "./lib/sort";
import { applyAlwaysOnTop } from "./lib/window";
import { loadPriorities, savePriorities, type Priority } from "./lib/priority";
import { TitleStrip } from "./components/TitleStrip";
import { Widget } from "./screens/Widget";
import { Tasks } from "./screens/Tasks";
import { Composer } from "./components/Composer";
import { Settings } from "./screens/Settings";

function App() {
  const [screen, setScreen] = useState<Screen>("widget");
  const [selected, setSelected] = useState<
    Partial<Record<EngineId, LimitWindowKind>>
  >({});

  const [priorities, setPriorities] = useState(loadPriorities);

  // Tasks and meters come from the store now; nothing here is invented.
  const { tasks, error, add, setEngine, remove } = useTasks();
  const { meters, error: meterError } = useMeters();
  const { runs, starting, stopping, error: runsError, start, stop } = useRuns();

  useEffect(() => {
    savePriorities(priorities);
  }, [priorities]);
  const [preferences, setPreferences] = useState(loadPreferences);

  // Theme and accent are stamped on the root element so the token blocks in
  // index.css can key off them, and remembered across launches.
  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-widget-theme", preferences.theme);
    root.setAttribute("data-accent", preferences.accent);
    // "system" means stamp nothing and let prefers-color-scheme decide.
    if (preferences.mode === "system") root.removeAttribute("data-mode");
    else root.setAttribute("data-mode", preferences.mode);
    savePreferences(preferences);
  }, [preferences]);

  // Always on top is a real window call, applied on load as well as on change
  // so the setting survives a restart.
  useEffect(() => {
    void applyAlwaysOnTop(preferences.alwaysOnTop);
  }, [preferences.alwaysOnTop]);

  const groups = useMemo(() => groupMeters(meters ?? []), [meters]);

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
      (tasks ?? []).filter(
        (task) => task.status === "queued" || task.status === "running",
      ),
    [tasks],
  );
  const queued = active.length;

  // The row button reflects the real active run per engine, never a toggle.
  const activeRuns = useMemo(() => Object.values(runs), [runs]);
  const runByEngine = useMemo(() => {
    const map: Partial<Record<EngineId, string>> = {};
    for (const run of activeRuns) map[run.engine] = run.runId;
    return map;
  }, [activeRuns]);
  const running = useMemo(() => {
    const map: Partial<Record<EngineId, boolean>> = {};
    for (const engine of Object.keys(runByEngine) as EngineId[])
      map[engine] = true;
    return map;
  }, [runByEngine]);
  const busy = useMemo(() => {
    const map: Partial<Record<EngineId, boolean>> = {};
    for (const run of activeRuns) {
      if (stopping[run.runId] === true) map[run.engine] = true;
    }
    for (const engine of Object.keys(starting) as EngineId[]) {
      if (starting[engine] === true) map[engine] = true;
    }
    return map;
  }, [activeRuns, starting, stopping]);

  // Working means a run is active, even if its meter just hit exhausted.
  const live = activeRuns.length;

  const problem = error ?? meterError ?? runsError;

  const status =
    screen === "widget"
      ? `${queued} queued · ${live === 0 ? "paused" : `${live} ${live === 1 ? "engine" : "engines"} working`}`
      : `${queued} queued`;

  // `undefined` until it resolves, never "": homeDir() is a round-trip, and an
  // empty string here would let a fast typer submit a task with no folder,
  // which the contract forbids. The composer disables send until it settles.
  const [folder, setFolder] = useState<string | undefined>(undefined);
  useEffect(() => {
    void defaultFolder(active[active.length - 1]?.folder).then(setFolder);
  }, [active]);

  const selectWindow = (engine: EngineId, kind: LimitWindowKind) =>
    setSelected((current) => ({ ...current, [engine]: kind }));

  const toggleRun = (engine: EngineId) => {
    const runId = runByEngine[engine];
    if (runId !== undefined) void stop(runId);
    else void start(engine);
  };
  const stopById = (runId: string) => void stop(runId);

  return (
    <main className="widget">
      <TitleStrip
        title={SCREEN_HEADING[screen]}
        status={status}
        screen={screen}
        onOpen={setScreen}
      />

      <div className="body">
        {problem !== null && (
          <p className="banner" role="alert">
            {problem}
          </p>
        )}

        {screen === "widget" && (
          <Widget
            groups={groups}
            selected={selected}
            running={running}
            busy={busy}
            now={now}
            nowPlaying={activeRuns}
            tasks={tasks}
            stopping={stopping}
            onSelectWindow={selectWindow}
            onToggleRun={toggleRun}
            onStopRun={stopById}
          />
        )}
        {screen === "tasks" && (
          <Tasks
            tasks={active}
            loading={tasks === null}
            priorities={priorities}
            sort={preferences.sort}
            onEngine={(id, engine) => void setEngine(id, engine)}
            onRemove={(id) => void remove(id)}
            onPriority={(id, priority: Priority) =>
              setPriorities((current) => ({ ...current, [id]: priority }))
            }
            onSort={(sort: Sort) =>
              setPreferences((current) => ({ ...current, sort }))
            }
          />
        )}
        {screen === "settings" && (
          <Settings
            preferences={preferences}
            onTheme={(theme: Theme) =>
              setPreferences((current) => ({ ...current, theme }))
            }
            onMode={(mode: Mode) =>
              setPreferences((current) => ({ ...current, mode }))
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

      {screen === "tasks" && (
        <Composer
          folder={folder}
          onSubmit={(prompt) => {
            if (folder !== undefined) void add(prompt, folder);
          }}
        />
      )}
    </main>
  );
}

export default App;
