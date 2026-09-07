import { describe, expect, test } from "vitest";
import { message } from "./errors";

describe("message", () => {
  test("a rejected command's string comes through as-is", () => {
    expect(message("database is locked")).toBe("database is locked");
  });

  test("an Error keeps its own message rather than becoming 'Unknown error'", () => {
    expect(message(new Error("invoke failed"))).toBe("invoke failed");
  });

  test("an Error with no message still says something", () => {
    expect(message(new Error(""))).toBe("Unknown error");
  });

  test.each([[null], [undefined], [42], [{}]])("%s -> Unknown error", (v) => {
    expect(message(v)).toBe("Unknown error");
  });
});
