import type { EngineId } from "../types";
import { ENGINE_LABEL, ENGINE_ORDER } from "../lib/engines";
import {
  ACCENTS,
  ACCENT_SWATCH,
  MODES,
  MODE_LABEL,
  THEMES,
  THEME_LABEL,
  type Accent,
  type Mode,
  type Preferences,
  type Theme,
} from "../lib/preferences";
import { SORTS, SORT_LABEL, type Sort } from "../lib/sort";
import { Section } from "../components/Section";

type Props = {
  preferences: Preferences;
  onTheme: (theme: Theme) => void;
  onMode: (mode: Mode) => void;
  onAccent: (accent: Accent) => void;
  onToggleAlwaysOnTop: () => void;
  onSort: (sort: Sort) => void;
  onToggleEngine: (engine: EngineId) => void;
};

export function Settings({
  preferences,
  onTheme,
  onMode,
  onAccent,
  onToggleAlwaysOnTop,
  onSort,
  onToggleEngine,
}: Props) {
  // One definition, rendered under whichever title the mode calls for: the
  // simple view must be byte-for-byte what shipped, so this cannot be a
  // separate copy that drifts.
  const look = (
    <>
      <div className="setrow">
        <span className="k">
          Always on top
          <span className="hint">keeps the widget above other windows</span>
        </span>
        <button
          type="button"
          className="toggle"
          aria-pressed={preferences.alwaysOnTop}
          aria-label="Always on top"
          onClick={onToggleAlwaysOnTop}
        />
      </div>

      <div className="setblock">
        <span className="k">Mode</span>
        <div className="modeseg" role="group" aria-label="Mode">
          {MODES.map((mode) => (
            <button
              key={mode}
              type="button"
              aria-pressed={preferences.mode === mode}
              onClick={() => onMode(mode)}
            >
              {MODE_LABEL[mode]}
            </button>
          ))}
        </div>
      </div>

      <div className="setrow">
        <span className="k">Accent</span>
        <span className="swatches" role="group" aria-label="Accent colour">
          {ACCENTS.map((accent) => (
            <button
              key={accent}
              type="button"
              className="swatch"
              style={{ "--sw": ACCENT_SWATCH[accent] } as React.CSSProperties}
              aria-pressed={preferences.accent === accent}
              aria-label={accent}
              onClick={() => onAccent(accent)}
            />
          ))}
        </span>
      </div>

      <div className="setblock">
        <span className="k">Theme</span>
        <div className="themegrid" role="group" aria-label="Theme">
          {THEMES.map((theme) => (
            <button
              key={theme}
              type="button"
              className="tbtn"
              aria-pressed={preferences.theme === theme}
              onClick={() => onTheme(theme)}
            >
              {THEME_LABEL[theme]}
            </button>
          ))}
        </div>
      </div>
    </>
  );

  if (!preferences.pro) {
    return (
      <div className="stack">
        <Section title="Look & Feel" hint="mode, accent, theme">
          {look}
        </Section>
      </div>
    );
  }

  return (
    <div className="stack">
      <Section title="Colour & Theme" hint="mode, accent, theme">
        {look}
      </Section>

      <Section title="Tasks" hint="how the queue is ordered">
        <div className="setblock">
          <span className="k">
            Default sort
            <span className="hint">what the queue opens on</span>
          </span>
          <div className="modeseg" role="group" aria-label="Default sort">
            {SORTS.map((sort) => (
              <button
                key={sort}
                type="button"
                aria-pressed={preferences.sort === sort}
                onClick={() => onSort(sort)}
              >
                {SORT_LABEL[sort]}
              </button>
            ))}
          </div>
        </div>
      </Section>

      <Section title="LLMs" hint="which engines you use">
        {ENGINE_ORDER.map((engine) => {
          const on = !preferences.hiddenEngines.includes(engine);
          return (
            <div className="setrow" key={engine}>
              <span className="k">{ENGINE_LABEL[engine]}</span>
              <button
                type="button"
                className="toggle"
                aria-pressed={on}
                aria-label={ENGINE_LABEL[engine]}
                onClick={() => onToggleEngine(engine)}
              />
            </div>
          );
        })}
      </Section>
    </div>
  );
}
