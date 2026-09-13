import assert from "node:assert/strict";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { tryRunSlashCommand } from "./slashCommands.js";

const ctx = { cwd: mkdtempSync(join(tmpdir(), "gosling-slash-")) };

test("/exit and /quit exit locally", () => {
  assert.deepEqual(tryRunSlashCommand("/exit", ctx), {
    handled: true,
    exit: true,
  });
  assert.deepEqual(tryRunSlashCommand("  /QUIT ", ctx), {
    handled: true,
    exit: true,
  });
});

test("/help and /? show the command list locally", () => {
  for (const input of ["/help", "/?"]) {
    const result = tryRunSlashCommand(input, ctx);
    assert.ok(result.handled && "message" in result && result.message);
    assert.match(result.message, /\/diff/);
    assert.match(result.message, /\/exit/);
  }
});

test("unknown commands, typos and a bare slash are refused locally", () => {
  for (const input of ["/eixt", "/halp", "/model", "/mode auto", "/"]) {
    const result = tryRunSlashCommand(input, ctx);
    assert.ok(
      result.handled && "message" in result && result.message,
      `expected ${input} to be handled locally`,
    );
    assert.match(result.message, /unknown command/);
    assert.match(result.message, /\/help/);
  }
});

test("prompts that start with a filesystem path still go to the model", () => {
  assert.deepEqual(
    tryRunSlashCommand("/usr/local/bin/tool fails, why?", ctx),
    { handled: false },
  );
  assert.deepEqual(tryRunSlashCommand("explain /exit", ctx), {
    handled: false,
  });
});

test("/diff outside a git repository keeps its existing message", () => {
  assert.deepEqual(tryRunSlashCommand("/diff", ctx), {
    handled: true,
    message: `not a git repository: ${ctx.cwd}`,
  });
});
