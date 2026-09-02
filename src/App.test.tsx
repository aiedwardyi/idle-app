import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test, vi } from "vitest";
import App from "./App";

const row = (name: string) =>
  screen
    .getByText(name, { selector: ".mname" })
    .closest(".meter-row") as HTMLElement;

describe("widget shell", () => {
  test("shows one row per engine", () => {
    render(<App />);
    for (const name of ["Claude", "Codex", "Antigravity", "Grok"]) {
      expect(screen.getByText(name)).toBeInTheDocument();
    }
  });

  test("starts on the widget screen, paused", () => {
    render(<App />);
    expect(screen.getByText(/3 queued · paused/i)).toBeInTheDocument();
  });
});

describe("window dragging", () => {
  test("the title strip is a drag region and its text does not block it", () => {
    render(<App />);
    const strip = document.querySelector("[data-tauri-drag-region]");
    expect(strip).toBeInTheDocument();

    // the brand text sits inside the strip, so it must not take the pointer
    const brand = strip?.querySelector(".brand");
    expect(brand).toBeInTheDocument();

    // the action buttons are siblings of the text, so they stay clickable
    expect(
      strip?.querySelector('.actions [aria-label="Settings"]'),
    ).toBeInTheDocument();
  });
});

describe("the reset countdown", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  test("advances on its own, with no interaction", async () => {
    vi.useFakeTimers();
    render(<App />);

    const resets = () =>
      row("Claude").querySelector(".resets")?.textContent ?? "";
    const before = resets();
    expect(before).toMatch(/left/);

    await act(async () => {
      vi.advanceTimersByTime(60_000);
    });

    expect(resets()).not.toBe(before);
  });
});

describe("screen navigation", () => {
  test("three tabs, always visible, with the active one marked", () => {
    render(<App />);
    for (const name of ["Meters", "Queue", "Settings"]) {
      expect(screen.getByLabelText(name)).toBeInTheDocument();
    }
    expect(screen.getByLabelText("Meters")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  test("the queue tab opens the queue and the meters tab returns", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByLabelText("Queue"));
    expect(
      screen.getByText("Add retry to the sync worker"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Claude", { selector: ".mname" }),
    ).not.toBeInTheDocument();

    // finished work is history, not queue
    expect(
      screen.queryByText("Fix flaky snapshot on Windows CI"),
    ).not.toBeInTheDocument();

    // there has to be a visible way back — the old build only toggled the
    // same icon, which works but tells the user nothing
    await user.click(screen.getByLabelText("Meters"));
    expect(
      screen.getByText("Claude", { selector: ".mname" }),
    ).toBeInTheDocument();
  });

  test("the settings tab opens settings", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByLabelText("Settings"));
    expect(screen.getByText("Always on top")).toBeInTheDocument();
    expect(screen.getByText("Theme")).toBeInTheDocument();
    expect(screen.getByText("Accent")).toBeInTheDocument();
  });
});

describe("preferences", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  test("picking a theme stamps the root and remembers it", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByLabelText("Settings"));
    await user.click(screen.getByRole("button", { name: "Console" }));

    expect(document.documentElement).toHaveAttribute(
      "data-widget-theme",
      "console",
    );
    expect(window.localStorage.getItem("idle.preferences")).toContain(
      "console",
    );
  });

  test("always on top persists", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByLabelText("Settings"));
    await user.click(screen.getByLabelText("Always on top"));

    expect(screen.getByLabelText("Always on top")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(window.localStorage.getItem("idle.preferences")).toContain(
      '"alwaysOnTop":true',
    );
  });

  test("picking an accent stamps the root", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByLabelText("Settings"));
    await user.click(screen.getByLabelText("teal"));

    expect(document.documentElement).toHaveAttribute("data-accent", "teal");
  });
});

describe("per-engine transport", () => {
  test("play affects only the engine it belongs to", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(
      within(row("Codex")).getByLabelText("Work the queue with Codex"),
    );

    expect(screen.getByText(/1 engine working/i)).toBeInTheDocument();
    expect(
      within(row("Codex")).getByLabelText("Pause Codex"),
    ).toBeInTheDocument();
    expect(
      within(row("Claude")).getByLabelText("Work the queue with Claude"),
    ).toBeInTheDocument();
  });
});

describe("window switch", () => {
  test("switching a window changes that row only", async () => {
    const user = userEvent.setup();
    render(<App />);

    const claude = row("Claude");
    const pct = () => claude.querySelector(".mpct")?.textContent ?? "";
    const before = pct();

    await user.click(within(claude).getByRole("button", { name: "7d" }));

    expect(pct()).not.toBe(before);
    expect(within(claude).getByRole("button", { name: "7d" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  test("single-window engines get a chip, not a switch", () => {
    render(<App />);
    const grok = row("Grok");
    expect(within(grok).queryByRole("group")).not.toBeInTheDocument();
    expect(within(grok).getByText("7d")).toBeInTheDocument();
  });
});
