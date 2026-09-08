import type { EngineId, LimitWindowKind } from "../types";
import { ENGINE_LABEL, WINDOW_LABEL } from "../lib/engines";
import {
  ACCENTS,
  ACCENT_SWATCH,
  BARS,
  BAR_LABEL,
  DENSITIES,
  DENSITY_LABEL,
  FONTS,
  FONT_LABEL,
  MODES,
  MODE_LABEL,
  OPACITY_MAX,
  OPACITY_MIN,
  RADII,
  RADIUS_LABEL,
  THEMES,
  THEME_LABEL,
  THRESHOLD_MAX,
  THRESHOLD_MIN,
  WINDOW_SIZES,
  WINDOW_SIZE_LABEL,
  type Preferences,
} from "../lib/preferences";
import { SORTS, SORT_LABEL } from "../lib/sort";
import { Section } from "../components/Section";

type Props = {
  preferences: Preferences;
  /** Windows each engine actually reports, so the picker offers real options. */
  windows: Partial<Record<EngineId, LimitWindowKind[]>>;
  onChange: (patch: Partial<Preferences>) => void;
};

/** Settings only exists in pro mode, so everything here is a pro control. */
export function Settings({ preferences, windows, onChange }: Props) {
  const p = preferences;

  const toggle = (
    label: string,
    hint: string,
    value: boolean,
    patch: (next: boolean) => Partial<Preferences>,
  ) => (
    <div className="setrow">
      <span className="k">
        {label}
        <span className="hint">{hint}</span>
      </span>
      <button
        type="button"
        className="toggle"
        aria-pressed={value}
        aria-label={label}
        onClick={() => onChange(patch(!value))}
      />
    </div>
  );

  const segment = <T extends string>(
    label: string,
    options: readonly T[],
    labels: Record<T, string>,
    value: T,
    patch: (next: T) => Partial<Preferences>,
    hint?: string,
  ) => (
    <div className="setblock">
      <span className="k">
        {label}
        {hint !== undefined && <span className="hint">{hint}</span>}
      </span>
      <div className="modeseg" role="group" aria-label={label}>
        {options.map((option) => (
          <button
            key={option}
            type="button"
            aria-pressed={value === option}
            onClick={() => onChange(patch(option))}
          >
            {labels[option]}
          </button>
        ))}
      </div>
    </div>
  );

  const slider = (
    label: string,
    hint: string,
    value: number,
    min: number,
    max: number,
    patch: (next: number) => Partial<Preferences>,
  ) => (
    <div className="setblock">
      <span className="k">
        {label}
        <span className="hint">{hint}</span>
      </span>
      <div className="sliderrow">
        <input
          type="range"
          className="slider"
          min={min}
          max={max}
          value={value}
          aria-label={label}
          onChange={(event) => onChange(patch(Number(event.target.value)))}
        />
        <span className="num">{value}%</span>
      </div>
    </div>
  );

  return (
    <div className="stack">
      <Section title="Colour & Theme" hint="mode, accent, shape">
        {toggle(
          "Always on top",
          "keeps the widget above other windows",
          p.alwaysOnTop,
          (alwaysOnTop) => ({ alwaysOnTop }),
        )}

        {segment(
          "Mode",
          MODES,
          MODE_LABEL,
          p.mode,
          (mode) => ({ mode }),
          p.autoDark ? "overridden by auto dark" : undefined,
        )}

        {toggle(
          "Auto dark",
          "dark from 7pm, light from 7am",
          p.autoDark,
          (autoDark) => ({ autoDark }),
        )}

        <div className="setrow">
          <span className="k">Accent</span>
          <span className="swatches" role="group" aria-label="Accent colour">
            {ACCENTS.map((accent) => (
              <button
                key={accent}
                type="button"
                className="swatch"
                style={{ "--sw": ACCENT_SWATCH[accent] } as React.CSSProperties}
                aria-pressed={p.accentCustom === null && p.accent === accent}
                aria-label={accent}
                onClick={() => onChange({ accent, accentCustom: null })}
              />
            ))}
            <input
              type="color"
              className="colorpick"
              value={p.accentCustom ?? ACCENT_SWATCH[p.accent]}
              aria-label="Custom accent"
              onChange={(event) =>
                onChange({ accentCustom: event.target.value })
              }
            />
            {p.accentCustom !== null && (
              <button
                type="button"
                className="linkbtn"
                aria-label="Reset accent"
                onClick={() => onChange({ accentCustom: null })}
              >
                reset
              </button>
            )}
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
                aria-pressed={p.theme === theme}
                onClick={() => onChange({ theme })}
              >
                {THEME_LABEL[theme]}
              </button>
            ))}
          </div>
        </div>

        {slider(
          "Opacity",
          "how much wallpaper shows through",
          p.opacity,
          OPACITY_MIN,
          OPACITY_MAX,
          (opacity) => ({ opacity }),
        )}

        {segment("Density", DENSITIES, DENSITY_LABEL, p.density, (density) => ({
          density,
        }))}
        {segment("Font", FONTS, FONT_LABEL, p.font, (font) => ({ font }))}
        {segment("Corners", RADII, RADIUS_LABEL, p.radius, (radius) => ({
          radius,
        }))}
        {segment("Bar", BARS, BAR_LABEL, p.bar, (bar) => ({ bar }))}
        {segment(
          "Window",
          WINDOW_SIZES,
          WINDOW_SIZE_LABEL,
          p.windowSize,
          (windowSize) => ({ windowSize }),
        )}

        {toggle(
          "Meter footer",
          "the state, percent and token line",
          p.showFooter,
          (showFooter) => ({ showFooter }),
        )}
        {toggle(
          "Bars only",
          "hide engine names and footers",
          p.barsOnly,
          (barsOnly) => ({ barsOnly }),
        )}
      </Section>

      <Section title="Tasks" hint="order and what shows">
        {segment(
          "Default sort",
          SORTS,
          SORT_LABEL,
          p.sort,
          (sort) => ({ sort }),
          "manual lets you drag rows",
        )}
        {toggle(
          "Show history",
          "keep done and failed tasks in the list",
          p.showHistory,
          (showHistory) => ({ showHistory }),
        )}
        <div className="setrow">
          <span className="k">
            Templates
            <span className="hint">
              {p.templates.length === 0
                ? "save a prompt from the composer to reuse it"
                : `${p.templates.length} saved — manage them in the composer`}
            </span>
          </span>
          {p.templates.length > 0 && (
            <button
              type="button"
              className="linkbtn"
              aria-label="Clear templates"
              onClick={() => onChange({ templates: [] })}
            >
              clear
            </button>
          )}
        </div>
      </Section>

      <Section title="LLMs" hint="which, in what order">
        {p.engineOrder.map((engine, index) => {
          const kinds = windows[engine] ?? [];
          const on = !p.hiddenEngines.includes(engine);
          const move = (to: number) => {
            const next = [...p.engineOrder];
            next.splice(to, 0, ...next.splice(index, 1));
            onChange({ engineOrder: next });
          };

          return (
            <div className="setrow" key={engine}>
              <span className="k">{ENGINE_LABEL[engine]}</span>

              {kinds.length > 1 && (
                <select
                  className="winpick"
                  aria-label={`Default window for ${ENGINE_LABEL[engine]}`}
                  value={p.defaultWindow[engine] ?? kinds[0]}
                  onChange={(event) =>
                    onChange({
                      defaultWindow: {
                        ...p.defaultWindow,
                        [engine]: event.target.value as LimitWindowKind,
                      },
                    })
                  }
                >
                  {kinds.map((kind) => (
                    <option key={kind} value={kind}>
                      {WINDOW_LABEL[kind]}
                    </option>
                  ))}
                </select>
              )}

              <span className="ordbtns">
                <button
                  type="button"
                  disabled={index === 0}
                  aria-label={`Move ${ENGINE_LABEL[engine]} up`}
                  onClick={() => move(index - 1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  disabled={index === p.engineOrder.length - 1}
                  aria-label={`Move ${ENGINE_LABEL[engine]} down`}
                  onClick={() => move(index + 1)}
                >
                  ↓
                </button>
              </span>

              <button
                type="button"
                className="toggle"
                aria-pressed={on}
                aria-label={ENGINE_LABEL[engine]}
                onClick={() =>
                  onChange({
                    hiddenEngines: on
                      ? [...p.hiddenEngines, engine]
                      : p.hiddenEngines.filter((id) => id !== engine),
                  })
                }
              />
            </div>
          );
        })}

        {slider(
          "Tight at",
          "percent used before a meter warns",
          p.tight,
          THRESHOLD_MIN,
          THRESHOLD_MAX,
          (tight) => ({ tight, near: Math.max(tight + 1, p.near) }),
        )}
        {slider(
          "Near limit at",
          "and before it goes urgent",
          p.near,
          THRESHOLD_MIN,
          THRESHOLD_MAX,
          (near) => ({ near, tight: Math.min(near - 1, p.tight) }),
        )}
      </Section>
    </div>
  );
}
