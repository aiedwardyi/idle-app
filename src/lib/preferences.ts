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

const KEY = "idle.preferences";

export type Preferences = {
  theme: Theme;
  /** Light by default: the system setting is opt-in, not assumed. */
  mode: Mode;
  accent: Accent;
  alwaysOnTop: boolean;
  /** Queue sort order — a view preference, so it lives here. */
  sort: Sort;
};

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "glass",
  mode: "light",
  accent: "blue",
  alwaysOnTop: false,
  sort: "added",
};

function isTheme(value: unknown): value is Theme {
  return THEMES.includes(value as Theme);
}

function isAccent(value: unknown): value is Accent {
  return ACCENTS.includes(value as Accent);
}

function isMode(value: unknown): value is Mode {
  return MODES.includes(value as Mode);
}

function isSort(value: unknown): value is Sort {
  return SORTS.includes(value as Sort);
}

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
    const { theme, mode, accent, alwaysOnTop, sort } = parsed as Record<
      string,
      unknown
    >;
    return {
      theme: isTheme(theme) ? theme : DEFAULT_PREFERENCES.theme,
      mode: isMode(mode) ? mode : DEFAULT_PREFERENCES.mode,
      accent: isAccent(accent) ? accent : DEFAULT_PREFERENCES.accent,
      alwaysOnTop:
        typeof alwaysOnTop === "boolean"
          ? alwaysOnTop
          : DEFAULT_PREFERENCES.alwaysOnTop,
      sort: isSort(sort) ? sort : DEFAULT_PREFERENCES.sort,
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
