import { afterEach, describe, expect, test } from "vitest";
import {
  DEFAULT_PREFERENCES,
  effectiveMode,
  loadPreferences,
  savePreferences,
} from "./preferences";

const KEY = "idle.preferences";

afterEach(() => {
  window.localStorage.clear();
});

describe("loadPreferences", () => {
  test("defaults when nothing is stored", () => {
    expect(loadPreferences()).toEqual(DEFAULT_PREFERENCES);
  });

  test("round-trips a saved appearance", () => {
    const saved = {
      ...DEFAULT_PREFERENCES,
      theme: "console",
      mode: "dark",
      accent: "teal",
      alwaysOnTop: true,
      sort: "priority",
      pro: true,
      hiddenEngines: ["grok"],
      accentCustom: "#ff8800",
      opacity: 60,
      density: "tiny",
      font: "mono",
      radius: "pill",
      bar: "ticks",
      showFooter: false,
      windowSize: "tall",
      autoDark: true,
      barsOnly: true,
      showHistory: true,
      order: ["t3", "t1"],
      templates: ["Rotate the API keys"],
      engineOrder: ["grok", "claude", "codex", "antigravity"],
      tight: 55,
      near: 91,
      defaultWindow: { claude: "weekly" },
    } as const;
    savePreferences(saved);
    expect(loadPreferences()).toEqual(saved);
  });

  test("ignores unknown values rather than stamping them on the root", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ theme: "hologram", accent: "puce" }),
    );
    expect(loadPreferences()).toEqual(DEFAULT_PREFERENCES);
  });

  test("an unknown mode falls back to light", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ theme: "glass", mode: "sepia", accent: "blue" }),
    );
    expect(loadPreferences().mode).toBe("light");
  });

  test("survives malformed json", () => {
    window.localStorage.setItem(KEY, "{not json");
    expect(loadPreferences()).toEqual(DEFAULT_PREFERENCES);
  });

  test("a non-boolean alwaysOnTop falls back rather than pinning the window", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ theme: "glass", accent: "blue", alwaysOnTop: "yes" }),
    );
    expect(loadPreferences().alwaysOnTop).toBe(false);
  });

  test("survives a stored non-object", () => {
    window.localStorage.setItem(KEY, "42");
    expect(loadPreferences()).toEqual(DEFAULT_PREFERENCES);
  });

  test("keeps a valid half of a partly broken record", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ theme: "bento", accent: "puce" }),
    );
    expect(loadPreferences()).toEqual({
      ...DEFAULT_PREFERENCES,
      theme: "bento",
    });
  });

  test("pro is off unless it was explicitly stored as a boolean", () => {
    expect(loadPreferences().pro).toBe(false);
    window.localStorage.setItem(KEY, JSON.stringify({ pro: "yes" }));
    expect(loadPreferences().pro).toBe(false);
  });

  test("unknown engine ids are dropped from hiddenEngines", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ hiddenEngines: ["grok", "bard", 7, "grok"] }),
    );
    expect(loadPreferences().hiddenEngines).toEqual(["grok"]);
  });

  test("a non-array hiddenEngines hides nothing", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ hiddenEngines: "grok" }));
    expect(loadPreferences().hiddenEngines).toEqual([]);
  });

  test("only a six-digit hex survives as a custom accent", () => {
    // This string is written into a CSS custom property, so it is the one
    // stored value that could do more than look wrong.
    for (const bad of ["red", "#fff", "url(x)", "#12345g", 7]) {
      window.localStorage.setItem(KEY, JSON.stringify({ accentCustom: bad }));
      expect(loadPreferences().accentCustom).toBeNull();
    }
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ accentCustom: "#AABBCC" }),
    );
    expect(loadPreferences().accentCustom).toBe("#aabbcc");
  });

  test("opacity and thresholds are clamped, not trusted", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({ opacity: 5, tight: -20, near: 400 }),
    );
    const loaded = loadPreferences();
    expect(loaded.opacity).toBe(45);
    expect(loaded.tight).toBe(10);
    expect(loaded.near).toBe(99);
  });

  test("near limit can never sink below tight", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ tight: 80, near: 40 }));
    const loaded = loadPreferences();
    expect(loaded.near).toBeGreaterThan(loaded.tight);
  });

  test("a partial engine order is completed, not honoured as-is", () => {
    window.localStorage.setItem(KEY, JSON.stringify({ engineOrder: ["grok"] }));
    expect(loadPreferences().engineOrder).toEqual([
      "grok",
      "claude",
      "codex",
      "antigravity",
    ]);
  });

  test("junk in defaultWindow is dropped key by key", () => {
    window.localStorage.setItem(
      KEY,
      JSON.stringify({
        defaultWindow: { claude: "weekly", bard: "weekly", grok: "yearly" },
      }),
    );
    expect(loadPreferences().defaultWindow).toEqual({ claude: "weekly" });
  });
});

describe("effectiveMode", () => {
  const at = (hour: number) => new Date(2026, 8, 1, hour, 0, 0);

  test("is the chosen mode unless auto dark is on", () => {
    const base = { ...DEFAULT_PREFERENCES, mode: "light" as const, pro: true };
    expect(effectiveMode(base, at(22))).toBe("light");
    expect(effectiveMode({ ...base, autoDark: true }, at(22))).toBe("dark");
    expect(effectiveMode({ ...base, autoDark: true }, at(12))).toBe("light");
    expect(effectiveMode({ ...base, autoDark: true }, at(3))).toBe("dark");
  });

  test("auto dark is a pro setting, so it does nothing while pro is off", () => {
    const off = {
      ...DEFAULT_PREFERENCES,
      autoDark: true,
      pro: false,
      mode: "light" as const,
    };
    expect(effectiveMode(off, at(22))).toBe("light");
  });
});
