import { describe, expect, test } from "vitest";
import { isAbsolute } from "./folder";

describe("isAbsolute", () => {
  test.each([
    ["/Users/you/code/ledger", true],
    ["/", true],
    ["C:\\Users\\you\\code", true],
    ["C:/Users/you/code", true],
    ["z:\\projects", true],
    ["", false],
    ["code/ledger", false],
    ["./code", false],
    ["../code", false],
    ["~/code", false],
  ])("%s -> %s", (path, expected) => {
    expect(isAbsolute(path)).toBe(expected);
  });
});
