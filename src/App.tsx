import { useEffect, useMemo, useState } from "react";
import type { EngineId, LimitWindowKind, TaskSize } from "./types";
import { useTasks } from "./hooks/useTasks";
import { useMeters } from "./hooks/useMeters";
import { defaultFolder } from "./lib/folder";
import { groupMeters, levelFor, usedPct } from "./lib/meters";
import { SCREEN_HEADING, type Screen } from "./lib/screens";
import {
  DEFAULT_PREFERENCES,
  WINDOW_SIZE_PX,
  effectiveMode,
  loadPreferences,
  savePreferences,
  type Preferences,
} from "./lib/preferences";
import { applyAlwaysOnTop, applySize } from "./lib/window";
import { loadPriorities, savePriorities, type Priority } from "./lib/priority";
import { TitleStrip } from "./components/TitleStrip";
import { Widget } from "./screens/Widget";
import { Tasks } from "./screens/Tasks";
import { NO_FILTERS, type Filters } from "./lib/filters";
import { Composer } from "./components/Composer";
import { Settings } from "./screens/Settings";

function App() {
  const [requested, setScreen] = useState<Screen>("widget");
  const [selected, setSelected] = useState<
    Partial<Record<EngineId, LimitWindowKind>>
  >({});
  const [running, setRunning] = useState<Partial<Record<EngineId, boolean>>>(
    {},
  );

  const [priorities, setPriorities] = useState(loadPriorities);

  // Tasks and meters come from the store now; nothing here is invented.
  const {
    tasks,
    error,
    add,
    setEngine,
    setSize,
    setEngineMany,
    remove,
    removeMany,
  } = useTasks();
  const { meters, error: meterError } = useMeters();

  useEffect(() => {
    savePriorities(priorities);
  }, [priorities]);
  const [preferences, setPreferences] = useState(loadPreferences);
  const pro = preferences.pro;

  const update = (patch: Partial<Preferences>) =>
    setPreferences((current) => ({ ...current, ...patch }));

  // Leaving pro mode closes the settings screen with it: the tab is gone, so
  // staying there would strand the user with no way back. Derived rather than
  // corrected in an effect, so there is never a frame showing the dead screen.
  const screen: Screen =
    !pro && requested === "settings" ? "widget" : requested;

  // The reset countdowns are relative to now, so the clock has to advance on
  // its own — otherwise a row reads "resets in 2h 14m" until some unrelated
  // interaction happens to re-render it. A minute is the display granularity.
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(timer);
  }, []);

  // Auto dark resolves against the same ticking clock, so 7pm arrives on its
  // own rather than at the next click.
  const mode = effectiveMode(preferences, now);

  // Theme, accent and every pro shape option are stamped on the root element
  // so the token blocks and the pro layer in index.css can key off them.
  useEffect(() => {
    const root = document.documentElement;
    const p = preferences;
    root.setAttribute("data-widget-theme", p.theme);
    root.setAttribute("data-accent", p.accent);
    // "system" means stamp nothing and let prefers-color-scheme decide.
    if (mode === "system") root.removeAttribute("data-mode");
    else root.setAttribute("data-mode", mode);

    // Pro-only presentation. Off means no attribute at all, so none of the
    // pro selectors match and the shipped look is what renders.
    const shape: Record<string, string | null> = {
      "data-density": p.pro ? p.density : null,
      "data-font": p.pro ? p.font : null,
      "data-radius": p.pro ? p.radius : null,
      "data-bar": p.pro ? p.bar : null,
      "data-bars-only": p.pro && p.barsOnly ? "true" : null,
    };
    for (const [name, value] of Object.entries(shape)) {
      if (value === null) root.removeAttribute(name);
      else root.setAttribute(name, value);
    }

    // Opacity and a custom accent are values, not variants, so they ride on
    // inline custom properties. The hex is validated on load.
    if (p.pro && p.opacity !== DEFAULT_PREFERENCES.opacity) {
      root.style.setProperty("--w-opacity", `${p.opacity}%`);
    } else root.style.removeProperty("--w-opacity");

    if (p.pro && p.accentCustom !== null) {
      root.style.setProperty("--accent", p.accentCustom);
    } else root.style.removeProperty("--accent");

    savePreferences(p);
  }, [preferences, mode]);

  // Always on top and the size preset are real window calls, applied on load
  // as well as on change so the settings survive a restart.
  useEffect(() => {
    void applyAlwaysOnTop(preferences.alwaysOnTop);
  }, [preferences.alwaysOnTop]);

  useEffect(() => {
    const { w, h } = WINDOW_SIZE_PX[preferences.windowSize];
    void applySize(w, h);
  }, [preferences.windowSize]);

  const all = useMemo(() => groupMeters(meters ?? []), [meters]);

  // Hiding and reordering engines only apply while pro is on: turning pro off
  // is meant to give back the default app, not a version of it with rows
  // missing or shuffled.
  const groups = useMemo(() => {
    if (!pro) return all;
    const rank = new Map(
      preferences.engineOrder.map((engine, index) => [engine, index]),
    );
    return all
      .filter((group) => !preferences.hiddenEngines.includes(group.engine))
      .sort((a, b) => (rank.get(a.engine) ?? 0) - (rank.get(b.engine) ?? 0));
  }, [all, pro, preferences.engineOrder, preferences.hiddenEngines]);

  const levels = useMemo(
    () =>
      pro
        ? { tight: preferences.tight, near: preferences.near }
        : { tight: DEFAULT_PREFERENCES.tight, near: DEFAULT_PREFERENCES.near },
    [pro, preferences.tight, preferences.near],
  );

  /** The windows each engine reports, for the default-window picker. */
  const windows = useMemo(
    () =>
      Object.fromEntries(
        all.map((group) => [group.engine, group.windows.map((w) => w.window)]),
      ) as Partial<Record<EngineId, LimitWindowKind[]>>,
    [all],
  );

  // Finished, failed and discarded tasks are history, not queue. The header
  // count and the queue screen read from the same list so they cannot disagree.
  const queue = useMemo(
    () =>
      (tasks ?? []).filter(
        (task) =>
          task.status === "queued" ||
          task.status === "running" ||
          (pro && preferences.showHistory),
      ),
    [tasks, pro, preferences.showHistory],
  );
  const queued = useMemo(
    () =>
      (tasks ?? []).filter(
        (task) => task.status === "queued" || task.status === "running",
      ).length,
    [tasks],
  );

  const live = groups.filter((group) => {
    if (!running[group.engine]) return false;
    const window =
      selected[group.engine] ??
      preferences.defaultWindow[group.engine] ??
      group.windows[0].window;
    const meter =
      group.windows.find((w) => w.window === window) ?? group.windows[0];
    return levelFor(usedPct(meter), levels) !== "hit";
  }).length;

  const problem = error ?? meterError;

  const status =
    screen === "widget"
      ? `${queued} queued · ${live === 0 ? "paused" : `${live} ${live === 1 ? "engine" : "engines"} working`}`
      : `${queued} queued`;

  // `undefined` until it resolves, never "": homeDir() is a round-trip, and an
  // empty string here would let a fast typer submit a task with no folder,
  // which the contract forbids. The composer disables send until it settles.
  const [folder, setFolder] = useState<string | undefined>(undefined);
  useEffect(() => {
    const newest = queue.filter(
      (task) => task.status === "queued" || task.status === "running",
    );
    void defaultFolder(newest[newest.length - 1]?.folder).then(setFolder);
  }, [queue]);

  const [filters, setFilters] = useState<Filters>(NO_FILTERS);

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
        pro={pro}
        onOpen={setScreen}
        onTogglePro={() => update({ pro: !pro })}
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
            now={now}
            levels={levels}
            showFooter={!pro || preferences.showFooter}
            defaultWindow={pro ? preferences.defaultWindow : {}}
            onSelectWindow={selectWindow}
            onToggleRun={toggleRun}
          />
        )}
        {screen === "tasks" && (
          <Tasks
            tasks={queue}
            loading={tasks === null}
            priorities={priorities}
            sort={preferences.sort}
            pro={pro}
            order={preferences.order}
            filters={pro ? filters : NO_FILTERS}
            onFilters={setFilters}
            onEngine={(id, engine) => void setEngine(id, engine)}
            onSize={(id, size: TaskSize) => void setSize(id, size)}
            onRemove={(id) => void remove(id)}
            onRemoveMany={(ids) => void removeMany(ids)}
            onEngineMany={(ids, engine) => void setEngineMany(ids, engine)}
            onReorder={(order) => update({ order })}
            onPriority={(id, priority: Priority) =>
              setPriorities((current) => ({ ...current, [id]: priority }))
            }
            onSort={(sort) => update({ sort })}
          />
        )}
        {screen === "settings" && pro && (
          <Settings
            preferences={preferences}
            windows={windows}
            onChange={update}
          />
        )}
      </div>

      {screen === "tasks" && (
        <Composer
          folder={folder}
          pro={pro}
          templates={pro ? preferences.templates : []}
          onSubmit={(prompt, target) => void add(prompt, target)}
          onSaveTemplate={(prompt) =>
            update({
              templates: preferences.templates.includes(prompt)
                ? preferences.templates
                : [...preferences.templates, prompt],
            })
          }
          onDeleteTemplate={(prompt) =>
            update({
              templates: preferences.templates.filter(
                (saved) => saved !== prompt,
              ),
            })
          }
        />
      )}
    </main>
  );
}

export default App;
