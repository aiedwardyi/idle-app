export const THEMES = ["glass", "bento", "minimal", "console"] as const;
export const ACCENTS = ["blue", "teal", "green", "magenta"] as const;

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
  blue: "#5b9dff",
  teal: "#2dd4bf",
  green: "#4ade80",
  magenta: "#e879f9",
};

const KEY = "idle.preferences";

export type Preferences = {
  theme: Theme;
  accent: Accent;
  alwaysOnTop: boolean;
};

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "glass",
  accent: "blue",
  alwaysOnTop: false,
};

function isTheme(value: unknown): value is Theme {
  return THEMES.includes(value as Theme);
}

function isAccent(value: unknown): value is Accent {
  return ACCENTS.includes(value as Accent);
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
    const { theme, accent, alwaysOnTop } = parsed as Record<string, unknown>;
    return {
      theme: isTheme(theme) ? theme : DEFAULT_PREFERENCES.theme,
      accent: isAccent(accent) ? accent : DEFAULT_PREFERENCES.accent,
      alwaysOnTop:
        typeof alwaysOnTop === "boolean"
          ? alwaysOnTop
          : DEFAULT_PREFERENCES.alwaysOnTop,
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
