import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";
import App from "./App";

test("renders idle-app", () => {
  render(<App />);
  expect(screen.getByText("idle-app")).toBeInTheDocument();
});
