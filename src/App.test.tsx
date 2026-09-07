import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, test, vi } from "vitest";
import App from "./App";
import { emit, ipc } from "./test/ipc";

/** Renders and waits for list_tasks / get_meters to land. */
const renderApp = async () => {
  render(<App />);
  await screen.findByText("Claude", { selector: ".mname" });
};

const user = () => userEvent.setup();

const row = (name: string) =>
  screen
    .getByText(name, { selector: ".mname" })
    .closest(".meter-row") as HTMLElement;

describe("widget shell", () => {
  test("shows one row per engine", async () => {
    await renderApp();
    for (const name of ["Claude", "Codex", "Antigravity", "Grok"]) {
      expect(screen.getByText(name)).toBeInTheDocument();
    }
  });

  test("starts on the widget screen, paused", async () => {
    await renderApp();
    expect(screen.getByText(/3 queued · paused/i)).toBeInTheDocument();
  });
});

describe("window dragging", () => {
  test("the title strip is a drag region and its text does not block it", async () => {
    await renderApp();
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
    // flush the initial command promises without leaving fake timers
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    const resets = () =>
      row("Claude").querySelector(".resets")?.textContent ?? "";
    const before = resets();
    expect(before).toMatch(/left/);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000);
    });

    expect(resets()).not.toBe(before);
  });
});

describe("screen navigation", () => {
  test("three tabs, always visible, with the active one marked", async () => {
    await renderApp();
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
    await renderApp();

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
    await renderApp();

    await user.click(screen.getByLabelText("Settings"));

    // sections start collapsed
    expect(screen.getByText("Look & Feel")).toBeInTheDocument();
    expect(screen.queryByText("Always on top")).not.toBeVisible();

    await user.click(screen.getByText("Look & Feel"));
    expect(screen.getByText("Always on top")).toBeVisible();
    expect(screen.getByText("Theme")).toBeVisible();
    expect(screen.getByText("Mode")).toBeVisible();
    expect(screen.getByText("Accent")).toBeInTheDocument();
  });
});

describe("the composer", () => {
  test("typing and sending adds the task to the queue", async () => {
    const user = userEvent.setup();
    await renderApp();

    await user.click(screen.getByLabelText("Queue"));
    expect(screen.getByText(/3 queued/i)).toBeInTheDocument();

    await user.type(screen.getByLabelText("New task"), "Rotate the API keys");
    await user.click(screen.getByLabelText("Add to queue"));

    expect(screen.getByText("Rotate the API keys")).toBeInTheDocument();
    expect(screen.getByText(/4 queued/i)).toBeInTheDocument();
    expect(screen.getByLabelText("New task")).toHaveValue("");
  });

  test("send is disabled until there is something to send", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByLabelText("Queue"));

    expect(screen.getByLabelText("Add to queue")).toBeDisabled();
    await user.type(screen.getByLabelText("New task"), "   ");
    expect(screen.getByLabelText("Add to queue")).toBeDisabled();
  });

  test("Enter sends and Shift+Enter does not", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByLabelText("Queue"));

    const input = screen.getByLabelText("New task");
    await user.type(input, "First line{Shift>}{Enter}{/Shift}second line");
    expect(screen.getByText(/3 queued/i)).toBeInTheDocument();

    await user.type(input, "{Enter}");
    expect(screen.getByText(/4 queued/i)).toBeInTheDocument();
  });

  test("the composer only appears on the queue screen", async () => {
    const user = userEvent.setup();
    await renderApp();

    expect(screen.queryByLabelText("New task")).not.toBeInTheDocument();
    await user.click(screen.getByLabelText("Queue"));
    expect(screen.getByLabelText("New task")).toBeInTheDocument();
    await user.click(screen.getByLabelText("Settings"));
    expect(screen.queryByLabelText("New task")).not.toBeInTheDocument();
  });
});

describe("live IPC", () => {
  const call = (cmd: string) => ipc.calls.find((c) => c.cmd === cmd);

  test("reads the queue from list_tasks and leaves finished work out", async () => {
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));

    expect(call("list_tasks")).toBeDefined();
    expect(
      screen.getByText("Add retry to the sync worker"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Fix flaky snapshot on Windows CI"),
    ).not.toBeInTheDocument();
  });

  test("an empty store is an empty queue, not a broken one", async () => {
    ipc.tasks = [];
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));

    expect(screen.getByText("Nothing queued.")).toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  test("a rejected command reaches the UI instead of vanishing", async () => {
    ipc.fail.list_tasks = "database is locked";
    await renderApp();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "database is locked",
    );
  });

  test("the composer calls add_task with the contract's arguments", async () => {
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));
    await user().type(screen.getByLabelText("New task"), "Rotate the API keys");
    await user().click(screen.getByLabelText("Add to queue"));

    expect(await screen.findByText("Rotate the API keys")).toBeInTheDocument();
    expect(call("add_task")?.args).toEqual({
      prompt: "Rotate the API keys",
      folder: "/Users/you/code/ledger",
      size: "m",
      engine: { type: "auto" },
    });
  });

  test("a failed add surfaces and adds no row", async () => {
    ipc.fail.add_task = "disk full";
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));
    await user().type(screen.getByLabelText("New task"), "Nope");
    await user().click(screen.getByLabelText("Add to queue"));

    expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    expect(screen.queryByText("Nope")).not.toBeInTheDocument();
  });

  test("a relative folder never reaches add_task", async () => {
    // The button guards this, but the boundary must hold on its own.
    ipc.tasks = [{ ...ipc.tasks[0], folder: "code/ledger" }];
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));
    await user().type(screen.getByLabelText("New task"), "Nope");
    await user().click(screen.getByLabelText("Add to queue"));

    expect(await screen.findByRole("alert")).toHaveTextContent("absolute path");
    expect(ipc.calls.some((c) => c.cmd === "add_task")).toBe(false);
  });

  test("picking an engine calls update_task", async () => {
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));
    await user().selectOptions(
      screen.getByLabelText("Engine for Add retry to the sync worker"),
      "codex",
    );

    expect(call("update_task")?.args).toEqual({
      id: "t1",
      engine: { type: "fixed", engine: "codex" },
    });
  });

  test("removing a task calls delete_task and drops the row", async () => {
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));
    await user().click(
      screen.getByLabelText("Remove Draft the migration plan for v3"),
    );

    expect(call("delete_task")?.args).toEqual({ id: "t3" });
    expect(
      screen.queryByText("Draft the migration plan for v3"),
    ).not.toBeInTheDocument();
  });

  test("a meter_update replaces one window and leaves the rest alone", async () => {
    await renderApp();
    const before = row("Codex").querySelector(".mpct")?.textContent;

    await act(async () => {
      emit("meter_update", {
        engine: "claude",
        window: "fiveHour",
        used: { input: 1, output: 1, cache: 1 },
        capacityEst: 100,
        calibrated: true,
        remainingPct: 1,
        resetsAt: null,
      });
    });

    expect(row("Claude").querySelector(".mpct")?.textContent).toBe("99% used");
    expect(row("Codex").querySelector(".mpct")?.textContent).toBe(before);
  });

  test("with no task to inherit from, the folder falls back to home", async () => {
    ipc.tasks = [];
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));

    expect(
      await screen.findByText(/runs in \/Users\/you$/),
    ).toBeInTheDocument();
  });
});

describe("queue sorting", () => {
  test("sorting by priority reorders and persists the choice", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByLabelText("Queue"));

    // default is the order tasks arrived in
    expect(screen.getByRole("button", { name: "Added" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    await user.selectOptions(
      screen.getByLabelText("Priority for Draft the migration plan for v3"),
      "high",
    );
    await user.click(screen.getByRole("button", { name: "Priority" }));

    const first = document.querySelector(".task b")?.textContent;
    expect(first).toBe("Draft the migration plan for v3");
    expect(window.localStorage.getItem("idle.preferences")).toContain(
      '"sort":"priority"',
    );
  });

  test("sorting by engine groups fixed engines before auto", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByLabelText("Queue"));
    await user.click(screen.getByRole("button", { name: "Engine" }));

    // the Claude-pinned task leads; the auto ones fall to the end
    expect(document.querySelector(".task b")?.textContent).toBe(
      "Write tests for the CSV parser",
    );
  });
});

describe("preferences", () => {
  afterEach(() => {
    window.localStorage.clear();
    document.documentElement.removeAttribute("data-mode");
  });

  test("picking a theme stamps the root and remembers it", async () => {
    const user = userEvent.setup();
    await renderApp();

    await user.click(screen.getByLabelText("Settings"));
    await user.click(screen.getByText("Look & Feel"));
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
    await renderApp();

    await user.click(screen.getByLabelText("Settings"));
    await user.click(screen.getByText("Look & Feel"));
    await user.click(screen.getByLabelText("Always on top"));

    expect(screen.getByLabelText("Always on top")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(window.localStorage.getItem("idle.preferences")).toContain(
      '"alwaysOnTop":true',
    );
  });

  test("light is the default and dark is an explicit choice", async () => {
    const user = userEvent.setup();
    await renderApp();
    expect(document.documentElement).toHaveAttribute("data-mode", "light");

    await user.click(screen.getByLabelText("Settings"));
    await user.click(screen.getByText("Look & Feel"));
    await user.click(screen.getByRole("button", { name: "Dark" }));
    expect(document.documentElement).toHaveAttribute("data-mode", "dark");

    // "system" stamps nothing so prefers-color-scheme decides
    await user.click(screen.getByRole("button", { name: "System" }));
    expect(document.documentElement).not.toHaveAttribute("data-mode");
  });

  test("priority persists per task", async () => {
    const user = userEvent.setup();
    await renderApp();

    await user.click(screen.getByLabelText("Queue"));
    const select = screen.getByLabelText(
      "Priority for Add retry to the sync worker",
    );
    await user.selectOptions(select, "high");

    expect(select).toHaveValue("high");
    expect(window.localStorage.getItem("idle.priorities")).toContain("high");
  });

  test("picking an accent stamps the root", async () => {
    const user = userEvent.setup();
    await renderApp();

    await user.click(screen.getByLabelText("Settings"));
    await user.click(screen.getByText("Look & Feel"));
    await user.click(screen.getByLabelText("teal"));

    expect(document.documentElement).toHaveAttribute("data-accent", "teal");
  });
});

describe("per-engine transport", () => {
  test("play affects only the engine it belongs to", async () => {
    const user = userEvent.setup();
    await renderApp();

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
    await renderApp();

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

  test("single-window engines get a chip, not a switch", async () => {
    await renderApp();
    const grok = row("Grok");
    expect(within(grok).queryByRole("group")).not.toBeInTheDocument();
    expect(within(grok).getByText("7d")).toBeInTheDocument();
  });
});
