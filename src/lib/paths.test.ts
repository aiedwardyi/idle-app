import { describe, expect, test } from "vitest";
import { folderName } from "./paths";

describe("folderName", () => {
  test.each([
    ["/Users/you/code/ledger", "ledger"],
    ["C:\\Users\\you\\code\\ledger", "ledger"],
    ["/Users/you/code/ledger/", "ledger"],
    ["C:\\Users\\you\\code\\ledger\\", "ledger"],
    ["/Users/you//code///ledger", "ledger"],
    ["C:/Users/you/code/ledger", "ledger"],
    ["ledger", "ledger"],
  ])("%s -> %s", (input, expected) => {
    expect(folderName(input)).toBe(expected);
  });

  test("falls back to the input when there is no segment", () => {
    expect(folderName("/")).toBe("/");
    expect(folderName("")).toBe("");
  });
});
