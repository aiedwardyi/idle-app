import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";
import { addListener, handleInvoke, resetIpc } from "./ipc";

// vitest runs without globals, so React Testing Library's own auto-cleanup
// never registers. Without this, renders leak between tests in the same file.
afterEach(cleanup);

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
  getCurrentWindow: () => ({ setAlwaysOnTop: () => Promise.resolve() }),
}));

beforeEach(resetIpc);
