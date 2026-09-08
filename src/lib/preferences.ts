import type { EngineId, LimitWindowKind } from "../types";
import { ENGINE_ORDER } from "./engines";
import { SORTS, type Sort } from "./sort";

export const MODES = ["light", "dark", "system"] as const;
export type Mode = (typeof MODES)[number];
export const MODE_LABEL: Record<Mode, string> = {
  light: "Light",
  dark: "Dark",
  system: "System",
};

export const THEMES = ["glass", "bento", "minimal", "console"] as const;
export const ACCENTS = ["blue", "indigo", "teal", "magenta"] as const;

export type Theme = (typeof THEMES)[number];
export type Accent = (typeof ACCENTS)[number];

export const THEME_LABEL: Record<Theme, string> = {
  glass: "Glass",
  bento: "Bento",
  minimal: "Minimal",
  console: "Console",
};

/** Swatch colours for the picker. The rendered accent comes from CSS. */
export const ACCENT_SWATCH: Record<Accent, string> = {
  blue: "#0f7cd2",
  indigo: "#6d68d7",
  teal: "#0a8b8b",
  magenta: "#ad4fa7",
};

export const DENSITIES = ["comfortable", "compact", "tiny"] as const;
export type Density = (typeof DENSITIES)[number];
export const DENSITY_LABEL: Record<Density, string> = {
  comfortable: "Roomy",
  compact: "Compact",
  tiny: "Tiny",
};

export const FONTS = ["system", "mono", "rounded"] as const;
export type FontChoice = (typeof FONTS)[number];
export const FONT_LABEL: Record<FontChoice, string> = {
  // "Sans", not "System": the mode segment already has a System and two
  // buttons with the same word in one screen is a coin toss for the user.
  system: "Sans",
  mono: "Mono",
  rounded: "Rounded",
};

export const RADII = ["sharp", "soft", "pill"] as const;
export type Radius = (typeof RADII)[number];
export const RADIUS_LABEL: Record<Radius, string> = {
  sharp: "Sharp",
  soft: "Soft",
  pill: "Pill",
};

export const BARS = ["solid", "striped", "ticks"] as const;
export type BarStyle = (typeof BARS)[number];
export const BAR_LABEL: Record<BarStyle, string> = {
  solid: "Solid",
  striped: "Striped",
  ticks: "Ticks",
};

/** Logical size presets, applied with a real window call inside Tauri. */
export const WINDOW_SIZES = ["narrow", "standard", "tall"] as const;
export type WindowSize = (typeof WINDOW_SIZES)[number];
export const WINDOW_SIZE_LABEL: Record<WindowSize, string> = {
  narrow: "Narrow",
  standard: "Standard",
  tall: "Tall",
};
export const WINDOW_SIZE_PX: Record<WindowSize, { w: number; h: number }> = {
  narrow: { w: 320, h: 380 },
  standard: { w: 384, h: 436 },
  tall: { w: 384, h: 600 },
};

/** Widget surface opacity, as a percentage. 82 is the shipped glass value. */
export const OPACITY_MIN = 45;
export const OPACITY_MAX = 100;

/**
 * Severity thresholds, in percent used. Ordered and clamped on read so a
 * stored pair can never invert and make `tight` unreachable.
 */
export const THRESHOLD_MIN = 10;
export const THRESHOLD_MAX = 99;

const KEY = "idle.preferences";

export type Preferences = {
  theme: Theme;
  /** Light by default: the system setting is opt-in, not assumed. */
  mode: Mode;
  accent: Accent;
  alwaysOnTop: boolean;
  /** Queue sort order — a view preference, so it lives here. */
  sort: Sort;
  /**
   * Pro mode. Off is the shipped product: meters and queue, nothing to
   * configure. On unlocks the settings tab and everything below.
   */
  pro: boolean;

  // --- colour & theme ---------------------------------------------------
  /** A hex the user picked, overriding the four presets. Null means presets. */
  accentCustom: string | null;
  opacity: number;
  density: Density;
  font: FontChoice;
  radius: Radius;
  bar: BarStyle;
  /** The state / percent / tokens line under each meter. */
  showFooter: boolean;
  windowSize: WindowSize;
  /** Dark after 7pm, light again in the morning. Overrides `mode` while on. */
  autoDark: boolean;
  /** Bars and controls only — no engine names, no footers. */
  barsOnly: boolean;

  // --- tasks ------------------------------------------------------------
  /** Include done, failed and discarded tasks in the queue list. */
  showHistory: boolean;
  /** Task ids in the order the user dragged them, for `sort: "manual"`. */
  order: string[];
  /** Saved prompts the composer can insert. */
  templates: string[];

  // --- llms -------------------------------------------------------------
  /** Engines in the user's own display order. */
  engineOrder: EngineId[];
  /** Engines switched off. Only honoured while `pro` is on. */
  hiddenEngines: EngineId[];
  /** Percent used at which a meter starts reading "tight". */
  tight: number;
  /** ...and "near limit". Always above `tight`. */
  near: number;
  /** The window each engine opens on, when it has more than one. */
  defaultWindow: Partial<Record<EngineId, LimitWindowKind>>;
};

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "glass",
  mode: "light",
  accent: "blue",
  alwaysOnTop: false,
  sort: "added",
  pro: false,

  accentCustom: null,
  opacity: 82,
  density: "comfortable",
  font: "system",
  radius: "soft",
  bar: "solid",
  showFooter: true,
  windowSize: "standard",
  autoDark: false,
  barsOnly: false,

  showHistory: false,
  order: [],
  templates: [],

  engineOrder: [...ENGINE_ORDER],
  hiddenEngines: [],
  tight: 70,
  near: 88,
  defaultWindow: {},
};

const oneOf =
  <T extends string>(values: readonly T[]) =>
  (value: unknown): value is T =>
    values.includes(value as T);

const isTheme = oneOf(THEMES);
const isAccent = oneOf(ACCENTS);
const isMode = oneOf(MODES);
const isSort = oneOf(SORTS);
const isDensity = oneOf(DENSITIES);
const isFont = oneOf(FONTS);
const isRadius = oneOf(RADII);
const isBar = oneOf(BARS);
const isWindowSize = oneOf(WINDOW_SIZES);
const isEngineId = oneOf(ENGINE_ORDER);
const isWindowKind = oneOf(["fiveHour", "daily", "weekly"] as const);

const bool = (value: unknown, fallback: boolean): boolean =>
  typeof value === "boolean" ? value : fallback;

const num = (
  value: unknown,
  fallback: number,
  lo: number,
  hi: number,
): number =>
  typeof value === "number" && Number.isFinite(value)
    ? Math.min(hi, Math.max(lo, Math.round(value)))
    : fallback;

/** Six-digit hex only: this string is written straight into a CSS custom property. */
const HEX = /^#[0-9a-f]{6}$/i;
const hex = (value: unknown): string | null =>
  typeof value === "string" && HEX.test(value) ? value.toLowerCase() : null;

const strings = (value: unknown, cap: number): string[] =>
  Array.isArray(value)
    ? value
        .filter((item): item is string => typeof item === "string")
        .slice(0, cap)
    : [];

/** Unknown ids are dropped, so a renamed engine cannot stay hidden forever. */
const engineIds = (value: unknown): EngineId[] =>
  Array.isArray(value) ? [...new Set(value.filter(isEngineId))] : [];

/** Every engine exactly once: the user's order first, then anything new. */
const toEngineOrder = (value: unknown): EngineId[] => {
  const wanted = engineIds(value);
  return [...wanted, ...ENGINE_ORDER.filter((id) => !wanted.includes(id))];
};

const toDefaultWindow = (
  value: unknown,
): Partial<Record<EngineId, LimitWindowKind>> => {
  if (typeof value !== "object" || value === null) return {};
  const out: Partial<Record<EngineId, LimitWindowKind>> = {};
  for (const [key, kind] of Object.entries(value as Record<string, unknown>)) {
    if (isEngineId(key) && isWindowKind(kind)) out[key] = kind;
  }
  return out;
};

/**
 * Preferences live in localStorage, not the SQLite store: CONTRACT.md has no
 * settings table, and a view preference is not app data. Reads are defensive —
 * storage can be unavailable or hold anything.
 */
export function loadPreferences(): Preferences {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (raw === null) return DEFAULT_PREFERENCES;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) {
      return DEFAULT_PREFERENCES;
    }
    const p = parsed as Record<string, unknown>;
    const d = DEFAULT_PREFERENCES;

    const tight = num(p.tight, d.tight, THRESHOLD_MIN, THRESHOLD_MAX);
    // "near limit" has to stay worse than "tight" or the ramp reads backwards.
    const near = Math.max(
      tight + 1,
      num(p.near, d.near, THRESHOLD_MIN, THRESHOLD_MAX),
    );

    return {
      theme: isTheme(p.theme) ? p.theme : d.theme,
      mode: isMode(p.mode) ? p.mode : d.mode,
      accent: isAccent(p.accent) ? p.accent : d.accent,
      alwaysOnTop: bool(p.alwaysOnTop, d.alwaysOnTop),
      sort: isSort(p.sort) ? p.sort : d.sort,
      pro: bool(p.pro, d.pro),

      accentCustom: hex(p.accentCustom),
      opacity: num(p.opacity, d.opacity, OPACITY_MIN, OPACITY_MAX),
      density: isDensity(p.density) ? p.density : d.density,
      font: isFont(p.font) ? p.font : d.font,
      radius: isRadius(p.radius) ? p.radius : d.radius,
      bar: isBar(p.bar) ? p.bar : d.bar,
      showFooter: bool(p.showFooter, d.showFooter),
      windowSize: isWindowSize(p.windowSize) ? p.windowSize : d.windowSize,
      autoDark: bool(p.autoDark, d.autoDark),
      barsOnly: bool(p.barsOnly, d.barsOnly),

      showHistory: bool(p.showHistory, d.showHistory),
      order: strings(p.order, 500),
      templates: strings(p.templates, 50),

      engineOrder: toEngineOrder(p.engineOrder),
      hiddenEngines: engineIds(p.hiddenEngines),
      tight,
      near: Math.min(THRESHOLD_MAX, near),
      defaultWindow: toDefaultWindow(p.defaultWindow),
    };
  } catch {
    return DEFAULT_PREFERENCES;
  }
}

export function savePreferences(preferences: Preferences): void {
  try {
    window.localStorage.setItem(KEY, JSON.stringify(preferences));
  } catch {
    // A widget that can't remember a colour is still a working widget.
  }
}

/**
 * The mode actually in force. Auto-dark is a schedule, so it can only be
 * resolved against a clock — the caller passes one rather than this reaching
 * for `new Date()` and becoming untestable.
 */
export function effectiveMode(preferences: Preferences, now: Date): Mode {
  if (!preferences.pro || !preferences.autoDark) return preferences.mode;
  const hour = now.getHours();
  return hour >= 19 || hour < 7 ? "dark" : "light";
}
