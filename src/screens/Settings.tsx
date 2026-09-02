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
import { Section } from "../components/Section";

type Props = {
  preferences: Preferences;
  onTheme: (theme: Theme) => void;
  onMode: (mode: Mode) => void;
  onAccent: (accent: Accent) => void;
  onToggleAlwaysOnTop: () => void;
};

export function Settings({
  preferences,
  onTheme,
  onMode,
  onAccent,
  onToggleAlwaysOnTop,
}: Props) {
  return (
    <div className="stack">
      <Section title="Look & Feel" hint="mode, accent, theme">
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
      </Section>
    </div>
  );
}
