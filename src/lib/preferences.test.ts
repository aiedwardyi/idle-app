import { afterEach, describe, expect, test } from "vitest";
import {
  DEFAULT_PREFERENCES,
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
      theme: "console",
      mode: "dark",
      accent: "teal",
      alwaysOnTop: true,
      sort: "priority",
      pro: true,
      hiddenEngines: ["grok"],
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
      theme: "bento",
      mode: DEFAULT_PREFERENCES.mode,
      accent: DEFAULT_PREFERENCES.accent,
      alwaysOnTop: DEFAULT_PREFERENCES.alwaysOnTop,
      sort: DEFAULT_PREFERENCES.sort,
      pro: DEFAULT_PREFERENCES.pro,
      hiddenEngines: DEFAULT_PREFERENCES.hiddenEngines,
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
});
