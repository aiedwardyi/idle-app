import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test, vi } from "vitest";
import App from "./App";

const row = (name: string) =>
  screen.getByText(name).closest(".meter-row") as HTMLElement;

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
    expect(before).toMatch(/resets in/);

    await act(async () => {
      vi.advanceTimersByTime(60_000);
    });

    expect(resets()).not.toBe(before);
  });
});

describe("screen navigation", () => {
  test("the queue button opens the queue and toggles back", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByLabelText("Task queue"));
    expect(screen.getByText("Queue")).toBeInTheDocument();
    expect(
      screen.getByText("Add retry to the sync worker"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Claude")).not.toBeInTheDocument();

    await user.click(screen.getByLabelText("Task queue"));
    expect(screen.getByText("Claude")).toBeInTheDocument();
  });

  test("the settings button opens settings", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByLabelText("Settings"));
    expect(screen.getByText("Always on top")).toBeInTheDocument();
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
    const before = within(claude).getByText(/%$/).textContent;

    await user.click(within(claude).getByRole("button", { name: "7d" }));

    expect(within(claude).getByText(/%$/).textContent).not.toBe(before);
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
