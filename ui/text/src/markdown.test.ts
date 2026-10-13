import assert from "node:assert/strict";
import test from "node:test";
import { renderMarkdown } from "./markdown.js";

test("markdown cache evicts old entries while preserving recent and changed renders", () => {
  const first = renderMarkdown("first", 40, "eviction-first");
  for (let index = 0; index < 250; index++) {
    renderMarkdown(`line ${index}`, 40, `eviction-${index}`);
  }
  assert.notEqual(renderMarkdown("first", 40, "eviction-first"), first);
  const recent = renderMarkdown("recent", 40, "recent");
  assert.equal(renderMarkdown("recent", 40, "recent"), recent);
  assert.notEqual(renderMarkdown("changed", 40, "recent"), recent);
  assert.notEqual(renderMarkdown("changed", 20, "recent"), recent);
});
