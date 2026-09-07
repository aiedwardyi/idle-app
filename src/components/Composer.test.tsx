import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, test, vi } from "vitest";
import { Composer } from "./Composer";

describe("Composer", () => {
  test("send stays disabled until the folder resolves", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    // `undefined` is the real first render: homeDir() is a round-trip.
    render(<Composer folder={undefined} onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText("New task"), "Rotate the API keys");

    expect(screen.getByLabelText("Add to queue")).toBeDisabled();
    expect(screen.getByText("finding a folder…")).toBeInTheDocument();

    // Enter must not slip past the disabled button either
    await user.type(screen.getByLabelText("New task"), "{Enter}");
    expect(onSubmit).not.toHaveBeenCalled();
  });

  test("send works once a folder is known", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(<Composer folder="/Users/you/code" onSubmit={onSubmit} />);

    await user.type(screen.getByLabelText("New task"), "Rotate the API keys");
    await user.click(screen.getByLabelText("Add to queue"));

    expect(onSubmit).toHaveBeenCalledWith("Rotate the API keys");
    expect(screen.getByText("runs in /Users/you/code")).toBeInTheDocument();
  });
});
