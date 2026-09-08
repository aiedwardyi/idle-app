import { act, fireEvent, render, screen, within } from "@testing-library/react";
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

/** Settings only exists in pro mode, so most of these have to unlock it. */
const openSettings = async (u: ReturnType<typeof userEvent.setup>) => {
  await u.click(screen.getByLabelText("Pro mode"));
  await u.click(screen.getByLabelText("Settings"));
};

const openLook = async (u: ReturnType<typeof userEvent.setup>) => {
  await openSettings(u);
  await u.click(screen.getByText("Colour & Theme"));
};

/** Pro is already on by this point, so only the tab needs clicking. */
const openSettingsAgain = async (u: ReturnType<typeof userEvent.setup>) => {
  await u.click(screen.getByLabelText("Settings"));
  await u.click(screen.getByText("Colour & Theme"));
};

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
      strip?.querySelector('.actions [aria-label="Queue"]'),
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
  test("two tabs by default, with the active one marked", async () => {
    await renderApp();
    for (const name of ["Meters", "Queue"]) {
      expect(screen.getByLabelText(name)).toBeInTheDocument();
    }
    // Nothing to configure without pro, so there is nothing to open.
    expect(screen.queryByLabelText("Settings")).not.toBeInTheDocument();
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
    await openSettings(user);

    // sections start collapsed
    expect(screen.getByText("Colour & Theme")).toBeInTheDocument();
    expect(screen.queryByText("Always on top")).not.toBeVisible();

    await user.click(screen.getByText("Colour & Theme"));
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
    await user.click(screen.getByLabelText("Meters"));
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
    ipc.tasks = [{ ...ipc.tasks[0], folder: "code/ledger" }];
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));
    await user().type(screen.getByLabelText("New task"), "Nope");

    // The composer refuses to arm, and says why rather than failing silently.
    expect(
      await screen.findByText("folder must be an absolute path"),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Add to queue")).toBeDisabled();
    await user().click(screen.getByLabelText("Add to queue"));
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

  test("an Error from a command keeps its message", async () => {
    // The contract rejects with strings, but a transport failure throws.
    ipc.fail.get_meters = new Error("invoke failed") as unknown as string;
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("invoke failed");
  });

  test("deleting then adding does not reuse a task id", async () => {
    await renderApp();
    await user().click(screen.getByLabelText("Queue"));

    await user().click(
      screen.getByLabelText("Remove Draft the migration plan for v3"),
    );
    await user().type(screen.getByLabelText("New task"), "Fresh task");
    await user().click(screen.getByLabelText("Add to queue"));

    await screen.findByText("Fresh task");
    const ids = ipc.tasks.map((t) => t.id);
    expect(new Set(ids).size).toBe(ids.length);
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

    await openLook(user);
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

    await openLook(user);
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

    await openLook(user);
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

    await openLook(user);
    await user.click(screen.getByLabelText("teal"));

    expect(document.documentElement).toHaveAttribute("data-accent", "teal");
  });
});

describe("pro mode", () => {
  afterEach(() => {
    window.localStorage.clear();
    for (const name of [
      "data-density",
      "data-font",
      "data-radius",
      "data-bar",
      "data-bars-only",
      "data-mode",
    ]) {
      document.documentElement.removeAttribute(name);
    }
    document.documentElement.removeAttribute("style");
  });

  test("off by default, with no settings tab and a switch that reads off", async () => {
    await renderApp();
    const sw = screen.getByLabelText("Pro mode");

    expect(sw).toHaveAttribute("aria-pressed", "false");
    // shaped like every other toggle in the app, not like a badge
    expect(sw.querySelector(".swtrack")).toBeInTheDocument();
    expect(screen.queryByLabelText("Settings")).not.toBeInTheDocument();
  });

  test("turning it on unlocks settings and remembers the choice", async () => {
    const u = userEvent.setup();
    await renderApp();
    await u.click(screen.getByLabelText("Pro mode"));

    expect(screen.getByLabelText("Pro mode")).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await u.click(screen.getByLabelText("Settings"));
    for (const group of ["Colour & Theme", "Tasks", "LLMs"]) {
      expect(screen.getByText(group)).toBeInTheDocument();
    }
    expect(window.localStorage.getItem("idle.preferences")).toContain(
      '"pro":true',
    );
  });

  test("leaving pro while on settings does not strand the user there", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openSettings(u);
    expect(screen.getByText("Colour & Theme")).toBeInTheDocument();

    await u.click(screen.getByLabelText("Pro mode"));

    // back on the meters, not on a screen with no tab to return from
    expect(screen.getByText("Claude", { selector: ".mname" })).toBeVisible();
    expect(screen.queryByText("Colour & Theme")).not.toBeInTheDocument();
  });

  test("the switch is reachable from every screen", async () => {
    const u = userEvent.setup();
    await renderApp();

    for (const tab of ["Queue", "Meters"]) {
      await u.click(screen.getByLabelText(tab));
      expect(screen.getByLabelText("Pro mode")).toBeInTheDocument();
    }
  });
});

describe("pro: colour & theme", () => {
  afterEach(() => {
    window.localStorage.clear();
    for (const name of [
      "data-density",
      "data-font",
      "data-radius",
      "data-bar",
      "data-bars-only",
      "data-mode",
    ]) {
      document.documentElement.removeAttribute(name);
    }
    document.documentElement.removeAttribute("style");
  });

  const root = () => document.documentElement;

  test("shape options stamp the root, and leaving pro takes them all off", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLook(u);

    await u.click(screen.getByRole("button", { name: "Tiny" }));
    await u.click(screen.getByRole("button", { name: "Mono" }));
    await u.click(screen.getByRole("button", { name: "Sharp" }));
    await u.click(screen.getByRole("button", { name: "Ticks" }));

    expect(root()).toHaveAttribute("data-density", "tiny");
    expect(root()).toHaveAttribute("data-font", "mono");
    expect(root()).toHaveAttribute("data-radius", "sharp");
    expect(root()).toHaveAttribute("data-bar", "ticks");

    // the shipped look is the absence of these, so they must actually go
    await u.click(screen.getByLabelText("Pro mode"));
    expect(root()).not.toHaveAttribute("data-density");
    expect(root()).not.toHaveAttribute("data-font");
    expect(root()).not.toHaveAttribute("data-radius");
    expect(root()).not.toHaveAttribute("data-bar");

    // ...and come back when it does, without being re-picked
    await u.click(screen.getByLabelText("Pro mode"));
    expect(root()).toHaveAttribute("data-density", "tiny");
  });

  test("the opacity slider drives the widget surface", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLook(u);

    const slider = screen.getByLabelText("Opacity");
    fireEvent.change(slider, { target: { value: "50" } });

    expect(root().style.getPropertyValue("--w-opacity")).toBe("50%");
    expect(screen.getByText("50%")).toBeInTheDocument();
  });

  test("a custom accent overrides the presets and can be given back", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLook(u);

    fireEvent.change(screen.getByLabelText("Custom accent"), {
      target: { value: "#ff8800" },
    });
    expect(root().style.getPropertyValue("--accent")).toBe("#ff8800");
    expect(screen.getByLabelText("blue")).toHaveAttribute(
      "aria-pressed",
      "false",
    );

    await u.click(screen.getByLabelText("Reset accent"));
    expect(root().style.getPropertyValue("--accent")).toBe("");
  });

  test("auto dark beats the mode segment, and only while pro is on", async () => {
    const u = userEvent.setup();
    vi.setSystemTime(new Date(2026, 8, 1, 22, 0, 0));
    try {
      await renderApp();
      await openLook(u);
      expect(root()).toHaveAttribute("data-mode", "light");

      await u.click(screen.getByLabelText("Auto dark"));
      expect(root()).toHaveAttribute("data-mode", "dark");

      await u.click(screen.getByLabelText("Pro mode"));
      expect(root()).toHaveAttribute("data-mode", "light");
    } finally {
      vi.useRealTimers();
    }
  });

  test("the footer and the names can be dropped from the rows", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLook(u);

    await u.click(screen.getByLabelText("Meter footer"));
    await u.click(screen.getByLabelText("Meters"));
    expect(document.querySelector(".mfoot")).not.toBeInTheDocument();

    await openSettingsAgain(u);
    await u.click(screen.getByLabelText("Bars only"));
    expect(root()).toHaveAttribute("data-bars-only", "true");
  });

  test("a size preset is a real window call", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLook(u);

    await u.click(screen.getByRole("button", { name: "Tall" }));

    expect(ipc.window.size.at(-1)).toEqual([384, 600]);
  });
});

describe("pro: tasks", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  const openQueue = async (u: ReturnType<typeof userEvent.setup>) => {
    await u.click(screen.getByLabelText("Pro mode"));
    await u.click(screen.getByLabelText("Queue"));
  };

  test("filter chips narrow the list and say so when nothing matches", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openQueue(u);

    await u.click(screen.getByRole("button", { name: "Claude" }));
    expect(
      screen.getByText("Write tests for the CSV parser"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("Add retry to the sync worker"),
    ).not.toBeInTheDocument();

    // a second axis can rule everything out, and that is not an error
    await u.click(screen.getByRole("button", { name: "queued" }));
    await u.click(screen.getByRole("button", { name: "running" }));
    expect(screen.getByText("Nothing matches those filters.")).toBeVisible();
  });

  test("history is off until asked for, and the header count ignores it", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openSettings(u);
    await u.click(screen.getByText("Tasks"));
    await u.click(screen.getByLabelText("Show history"));
    await u.click(screen.getByLabelText("Queue"));

    expect(
      screen.getByText("Fix flaky snapshot on Windows CI"),
    ).toBeInTheDocument();
    // done work is visible but it is not queued work
    expect(screen.getByText(/3 queued/i)).toBeInTheDocument();
  });

  test("a per-task size change goes through update_task", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openQueue(u);

    await u.selectOptions(
      screen.getByLabelText("Size for Add retry to the sync worker"),
      "l",
    );

    expect(ipc.calls.find((c) => c.cmd === "update_task")?.args).toEqual({
      id: "t1",
      size: "l",
    });
  });

  test("bulk selection reassigns the engine in one action", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openQueue(u);

    await u.click(screen.getByLabelText("Select Add retry to the sync worker"));
    await u.click(
      screen.getByLabelText("Select Draft the migration plan for v3"),
    );
    expect(screen.getByText("2 selected")).toBeVisible();

    await u.selectOptions(screen.getByLabelText("Engine for selected"), "grok");

    const updates = ipc.calls.filter((c) => c.cmd === "update_task");
    expect(updates).toHaveLength(2);
    expect(updates.map((c) => c.args.id).sort()).toEqual(["t1", "t3"]);
    expect(screen.queryByText("2 selected")).not.toBeInTheDocument();
  });

  test("bulk remove deletes every selected task and no others", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openQueue(u);

    await u.click(screen.getByLabelText("Select Add retry to the sync worker"));
    await u.click(screen.getByLabelText("Remove selected"));

    expect(ipc.calls.filter((c) => c.cmd === "delete_task")).toHaveLength(1);
    expect(
      screen.queryByText("Add retry to the sync worker"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText("Write tests for the CSV parser"),
    ).toBeInTheDocument();
  });

  test("manual sort only exists in pro, and a drag reorders and persists", async () => {
    const u = userEvent.setup();
    await renderApp();
    await u.click(screen.getByLabelText("Queue"));
    expect(
      screen.queryByRole("button", { name: "Manual" }),
    ).not.toBeInTheDocument();

    await u.click(screen.getByLabelText("Pro mode"));
    await u.click(screen.getByRole("button", { name: "Manual" }));

    const rows = () =>
      [...document.querySelectorAll(".task b")].map((n) => n.textContent);
    expect(rows()[0]).toBe("Add retry to the sync worker");

    const third = document.querySelectorAll(".task")[2];
    const first = document.querySelectorAll(".task")[0];
    fireEvent.dragStart(third);
    fireEvent.dragOver(first);
    fireEvent.drop(first);

    expect(rows()[0]).toBe("Draft the migration plan for v3");
    expect(window.localStorage.getItem("idle.preferences")).toContain(
      '"order":["t3"',
    );
  });

  test("a prompt can be saved, reused and forgotten", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openQueue(u);

    await u.type(screen.getByLabelText("New task"), "Rotate the API keys");
    await u.click(screen.getByLabelText("Save as template"));
    await u.click(screen.getByLabelText("Add to queue"));

    // the box is empty again, but the template is not
    expect(screen.getByLabelText("New task")).toHaveValue("");
    await u.click(screen.getByLabelText("Use template Rotate the API keys"));
    expect(screen.getByLabelText("New task")).toHaveValue(
      "Rotate the API keys",
    );

    await u.click(screen.getByLabelText("Forget template Rotate the API keys"));
    expect(
      screen.queryByLabelText("Use template Rotate the API keys"),
    ).not.toBeInTheDocument();
  });

  test("a per-task folder overrides the inherited one", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openQueue(u);

    await u.click(screen.getByLabelText("Change folder"));
    const field = screen.getByLabelText("Task folder");
    await u.clear(field);
    await u.type(field, "/Users/you/other");
    await u.type(screen.getByLabelText("New task"), "Elsewhere");
    await u.click(screen.getByLabelText("Add to queue"));

    expect(ipc.calls.find((c) => c.cmd === "add_task")?.args).toEqual({
      prompt: "Elsewhere",
      folder: "/Users/you/other",
      size: "m",
      engine: { type: "auto" },
    });
  });
});

describe("pro: llms", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  const openLlms = async (u: ReturnType<typeof userEvent.setup>) => {
    await openSettings(u);
    await u.click(screen.getByText("LLMs"));
  };

  test("switching an engine off drops its row; leaving pro brings it back", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLlms(u);
    await u.click(screen.getByLabelText("Grok"));

    await u.click(screen.getByLabelText("Meters"));
    expect(
      screen.queryByText("Grok", { selector: ".mname" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("Claude", { selector: ".mname" })).toBeVisible();

    await u.click(screen.getByLabelText("Pro mode"));
    expect(screen.getByText("Grok", { selector: ".mname" })).toBeVisible();
  });

  test("a hidden engine cannot be counted as working", async () => {
    const u = userEvent.setup();
    await renderApp();

    // t2 is pinned to Claude, so starting it makes Claude the live engine
    await u.click(screen.getByLabelText("Queue"));
    await u.click(screen.getByLabelText("Run Write tests for the CSV parser"));
    await u.click(screen.getByLabelText("Meters"));
    expect(screen.getByText(/1 engine working/i)).toBeInTheDocument();

    // hiding it must not leave a count with no row to explain it
    await openLlms(u);
    await u.click(screen.getByLabelText("Claude"));
    await u.click(screen.getByLabelText("Meters"));

    expect(screen.getByText(/paused/i)).toBeInTheDocument();
  });

  test("reordering moves the row, and only while pro is on", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLlms(u);

    await u.click(screen.getByLabelText("Move Grok up"));
    await u.click(screen.getByLabelText("Move Grok up"));
    await u.click(screen.getByLabelText("Move Grok up"));
    await u.click(screen.getByLabelText("Meters"));

    const names = () =>
      [...document.querySelectorAll(".mname")].map((n) => n.textContent);
    expect(names()[0]).toBe("Grok");

    await u.click(screen.getByLabelText("Pro mode"));
    expect(names()[0]).toBe("Claude");
  });

  test("moving the top engine up is not offered", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLlms(u);

    expect(screen.getByLabelText("Move Claude up")).toBeDisabled();
    expect(screen.getByLabelText("Move Grok down")).toBeDisabled();
  });

  test("the thresholds decide what a row calls itself", async () => {
    const u = userEvent.setup();
    await renderApp();

    // Claude sits at 74% used, which is "tight" at the shipped 70
    expect(within(row("Claude")).getByText("tight")).toBeInTheDocument();

    await openLlms(u);
    fireEvent.change(screen.getByLabelText("Tight at"), {
      target: { value: "80" },
    });
    await u.click(screen.getByLabelText("Meters"));
    expect(within(row("Claude")).getByText("ok")).toBeInTheDocument();

    // ...and the shipped thresholds come back with pro off
    await u.click(screen.getByLabelText("Pro mode"));
    expect(within(row("Claude")).getByText("tight")).toBeInTheDocument();
  });

  test("near limit cannot be dragged below tight", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLlms(u);

    fireEvent.change(screen.getByLabelText("Near limit at"), {
      target: { value: "40" },
    });

    const tight = screen.getByLabelText("Tight at") as HTMLInputElement;
    const near = screen.getByLabelText("Near limit at") as HTMLInputElement;
    expect(Number(near.value)).toBeGreaterThan(Number(tight.value));
  });

  test("a default window opens the row on it without a click", async () => {
    const u = userEvent.setup();
    await renderApp();
    await openLlms(u);

    await u.selectOptions(
      screen.getByLabelText("Default window for Claude"),
      "weekly",
    );
    await u.click(screen.getByLabelText("Meters"));

    expect(
      within(row("Claude")).getByRole("button", { name: "7d" }),
    ).toHaveAttribute("aria-pressed", "true");
  });
});

describe("running a task", () => {
  const call = (cmd: string) => ipc.calls.find((c) => c.cmd === cmd);

  const openQueue = async (u: ReturnType<typeof userEvent.setup>) => {
    await renderApp();
    await u.click(screen.getByLabelText("Queue"));
  };

  test("play on a task calls run_now with that task", async () => {
    const u = userEvent.setup();
    await openQueue(u);

    await u.click(screen.getByLabelText("Run Draft the migration plan for v3"));

    expect(call("run_now")?.args).toEqual({ taskId: "t3" });
    // the store claimed it, so the same button now stops it
    const button = await screen.findByLabelText(
      "Stop Draft the migration plan for v3",
    );
    await u.click(button);
    expect(call("stop_run")?.args).toEqual({ runId: "r1" });
  });

  test("a task running in another window offers neither start nor stop", async () => {
    const u = userEvent.setup();
    await openQueue(u);

    // t1 arrives `running` from the store, started by nobody in this window,
    // so there is no run id here to cancel and nothing to restart.
    expect(
      screen.getByLabelText("Add retry to the sync worker is already running"),
    ).toBeDisabled();
    expect(ipc.calls.some((c) => c.cmd === "run_now")).toBe(false);
  });

  test("a rejected run_now surfaces and starts nothing", async () => {
    const u = userEvent.setup();
    ipc.fail.run_now = "claude CLI not found";
    await openQueue(u);

    await u.click(screen.getByLabelText("Run Draft the migration plan for v3"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "claude CLI not found",
    );
    expect(screen.getByText(/3 queued/i)).toBeInTheDocument();
  });

  test("an engine is working only while a process is", async () => {
    const u = userEvent.setup();
    await openQueue(u);

    // t2 is pinned to Claude, so that is the engine that lights up
    await u.click(screen.getByLabelText("Run Write tests for the CSV parser"));
    await u.click(screen.getByLabelText("Meters"));

    expect(screen.getByText(/1 engine working/i)).toBeInTheDocument();
    expect(within(row("Claude")).getByLabelText("Stop Claude")).toBeEnabled();
    // ...and the ones with nothing in flight say so instead of offering play
    expect(
      within(row("Codex")).getByLabelText(
        "Codex is idle — press play on a task",
      ),
    ).toBeDisabled();
  });

  test("stopping calls stop_run with the run id", async () => {
    const u = userEvent.setup();
    await openQueue(u);
    await u.click(screen.getByLabelText("Run Write tests for the CSV parser"));
    await u.click(screen.getByLabelText("Meters"));

    await u.click(within(row("Claude")).getByLabelText("Stop Claude"));

    expect(call("stop_run")?.args).toEqual({ runId: "r1" });
  });

  test("a finished run clears the engine and re-reads the store", async () => {
    const u = userEvent.setup();
    await openQueue(u);
    await u.click(screen.getByLabelText("Run Write tests for the CSV parser"));
    await u.click(screen.getByLabelText("Meters"));
    expect(screen.getByText(/1 engine working/i)).toBeInTheDocument();

    const before = ipc.calls.filter((c) => c.cmd === "list_tasks").length;
    // The backend flips the task to done after the process exits, so the
    // truth is in the store, not in an optimistic guess here.
    ipc.tasks = ipc.tasks.map((t) =>
      t.id === "t2" ? { ...t, status: "done" as const } : t,
    );
    await act(async () => {
      emit("run_event", { type: "finished", runId: "r1", ok: true });
    });

    expect(screen.getByText(/paused/i)).toBeInTheDocument();
    expect(ipc.calls.filter((c) => c.cmd === "list_tasks").length).toBe(
      before + 1,
    );
    expect(screen.getByText(/2 queued/i)).toBeInTheDocument();
  });

  test("an error event surfaces and does not leave the engine working", async () => {
    const u = userEvent.setup();
    await openQueue(u);
    await u.click(screen.getByLabelText("Run Write tests for the CSV parser"));

    await act(async () => {
      emit("run_event", {
        type: "error",
        runId: "r1",
        message: "spawn failed",
      });
    });

    expect(await screen.findByRole("alert")).toHaveTextContent("spawn failed");
    await u.click(screen.getByLabelText("Meters"));
    expect(screen.getByText(/paused/i)).toBeInTheDocument();
  });

  test("usage from a real run reaches the bar it belongs to", async () => {
    // The blank meters people see are a store with nothing in it, not a
    // broken read: one meter_update is enough to fill the row in.
    await renderApp();
    ipc.meters = [
      {
        engine: "claude",
        window: "fiveHour",
        used: { input: 0, output: 0, cache: 0 },
        capacityEst: null,
        calibrated: false,
        remainingPct: null,
        resetsAt: null,
      },
    ];
    render(<App />);
    expect((await screen.findAllByText("no estimate")).length).toBeGreaterThan(
      0,
    );

    await act(async () => {
      emit("meter_update", {
        engine: "claude",
        window: "fiveHour",
        used: { input: 900_000, output: 100_000, cache: 0 },
        capacityEst: 2_000_000,
        calibrated: true,
        remainingPct: 50,
        resetsAt: null,
      });
    });

    expect(screen.getAllByText("50% used").length).toBeGreaterThan(0);
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
