import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";
import type { MeterState } from "../types";
import { MeterRow } from "./MeterRow";
import type { EngineMeters } from "../lib/meters";

const NOW = new Date("2026-09-01T12:00:00Z");

const meter = (over: Partial<MeterState> = {}): MeterState => ({
  engine: "claude",
  window: "fiveHour",
  used: { input: 1_000_000, output: 200_000, cache: 40_000 },
  capacityEst: 2_400_000,
  calibrated: true,
  remainingPct: 48,
  resetsAt: "2026-09-01T14:06:00Z",
  ...over,
});

const group = (windows: MeterState[]): EngineMeters => ({
  engine: "claude",
  windows,
});

const noop = () => {};

describe("MeterRow", () => {
  test("renders the engine, percentage and reset time", () => {
    render(
      <MeterRow
        group={group([meter()])}
        selected="fiveHour"
        running={false}
        now={NOW}
        onSelectWindow={noop}
        onToggleRun={noop}
      />,
    );

    expect(screen.getByText("Claude")).toBeInTheDocument();
    expect(screen.getByText("52%")).toBeInTheDocument();
    expect(screen.getByText(/2h 06m/)).toBeInTheDocument();
  });

  test("an exhausted engine cannot be started", () => {
    render(
      <MeterRow
        group={group([meter({ remainingPct: -22 })])}
        selected="fiveHour"
        running={false}
        now={NOW}
        onSelectWindow={noop}
        onToggleRun={noop}
      />,
    );

    expect(screen.getByText("limit hit")).toBeInTheDocument();
    expect(screen.getByText(/back in/)).toBeInTheDocument();
    // a clock would imply there is still time on the window
    expect(document.querySelector('[data-icon="blocked"]')).toBeInTheDocument();
    expect(document.querySelector('[data-icon="near"]')).toBeNull();
    expect(screen.getByLabelText("Claude has no headroom left")).toBeDisabled();
  });

  test("an uncalibrated estimate is marked, not stated as fact", () => {
    const { container } = render(
      <MeterRow
        group={group([
          meter({
            calibrated: false,
            remainingPct: null,
            capacityEst: 5_000_000,
          }),
        ])}
        selected="fiveHour"
        running={false}
        now={NOW}
        onSelectWindow={noop}
        onToggleRun={noop}
      />,
    );

    expect(screen.getByText("~25%")).toBeInTheDocument();
    // the "~" has to reach the capacity figure too, not just the percentage
    expect(container.textContent).toContain("~5.0M");
  });

  test("shows an em dash when there is no estimate at all", () => {
    render(
      <MeterRow
        group={group([meter({ remainingPct: null, capacityEst: null })])}
        selected="fiveHour"
        running={false}
        now={NOW}
        onSelectWindow={noop}
        onToggleRun={noop}
      />,
    );

    expect(screen.getByText("—")).toBeInTheDocument();
    expect(screen.getByText("no estimate")).toBeInTheDocument();
    expect(document.querySelector('[data-icon="unknown"]')).toBeInTheDocument();
    expect(document.querySelector('[data-icon="near"]')).toBeNull();
  });

  test("clicking a window option reports that window up", async () => {
    const user = userEvent.setup();
    const onSelectWindow = vi.fn();

    render(
      <MeterRow
        group={group([meter(), meter({ window: "weekly" })])}
        selected="fiveHour"
        running={false}
        now={NOW}
        onSelectWindow={onSelectWindow}
        onToggleRun={noop}
      />,
    );

    await user.click(screen.getByRole("button", { name: "7d" }));
    expect(onSelectWindow).toHaveBeenCalledWith("weekly");
  });

  test("falls back to the first window when the selection is unavailable", () => {
    render(
      <MeterRow
        group={group([meter({ window: "weekly" })])}
        selected="fiveHour"
        running={false}
        now={NOW}
        onSelectWindow={noop}
        onToggleRun={noop}
      />,
    );

    expect(screen.getByText("7d")).toBeInTheDocument();
  });
});
