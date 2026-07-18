import assert from "node:assert/strict";
import test from "node:test";

import { truncateTerminalText } from "./utils.js";

test("truncateTerminalText keeps output within an ASCII cell budget", () => {
  assert.equal(truncateTerminalText("123456789", 6), "12345…");
  assert.equal(truncateTerminalText("12345", 6), "12345");
});

test("truncateTerminalText flattens dynamic multi-line text", () => {
  assert.equal(
    truncateTerminalText("first\n  second\tthird", 18),
    "first second third",
  );
});

test("truncateTerminalText budgets non-ASCII characters conservatively", () => {
  assert.equal(truncateTerminalText("ab界cd", 5), "ab界…");
  assert.equal(truncateTerminalText("anything", 1), "…");
  assert.equal(truncateTerminalText("anything", 0), "");
});
