import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";
import { addListener, handleInvoke, ipc, resetIpc } from "./ipc";

// vitest runs without globals, so React Testing Library's own auto-cleanup
// never registers. Without this, renders leak between tests in the same file.
afterEach(cleanup);

// Preferences persist by design, which between tests means one test's theme —
// or its pro switch — silently becomes the next one's starting state. The
// root element carries the same state as attributes, so it is reset with it.
afterEach(() => {
  window.localStorage.clear();
  const root = document.documentElement;
  for (const name of root.getAttributeNames()) {
    if (name.startsWith("data-")) root.removeAttribute(name);
  }
  root.removeAttribute("style");
});

// The whole UI talks to Tauri through these three modules. Mocking them here
// rather than per-file means every test exercises the real command names.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) =>
    handleInvoke(cmd, args),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) =>
    Promise.resolve(addListener(event, (payload) => handler({ payload }))),
}));

vi.mock("@tauri-apps/api/path", () => ({
  homeDir: () => Promise.resolve("/Users/you"),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    setAlwaysOnTop: (value: boolean) => {
      ipc.window.alwaysOnTop.push(value);
      return Promise.resolve();
    },
    setSize: (size: { width: number; height: number }) => {
      ipc.window.size.push([size.width, size.height]);
      return Promise.resolve();
    },
  }),
  LogicalSize: class {
    constructor(
      public width: number,
      public height: number,
    ) {}
  },
}));

beforeEach(resetIpc);
