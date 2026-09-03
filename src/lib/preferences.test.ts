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
    savePreferences({
      theme: "console",
      mode: "dark",
      accent: "teal",
      alwaysOnTop: true,
      sort: "priority",
    });
    expect(loadPreferences()).toEqual({
      theme: "console",
      mode: "dark",
      accent: "teal",
      alwaysOnTop: true,
      sort: "priority",
    });
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
    });
  });
});
