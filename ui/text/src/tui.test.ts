import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { interactiveTerminalError } from "./utils.js";

const tuiEntry = fileURLToPath(new URL("./tui.tsx", import.meta.url));

test("interactive mode needs a TTY on stdin; --text mode does not", () => {
  assert.match(
    interactiveTerminalError(false, false) ?? "",
    /needs an interactive terminal/,
  );
  assert.equal(interactiveTerminalError(false, true), null);
  assert.equal(interactiveTerminalError(true, false), null);
});

test("tui with non-interactive stdin exits with one clear line instead of an Ink stack trace", () => {
  const result = spawnSync(process.execPath, ["--import", "tsx", tuiEntry], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, GOSLING_BINARY: "/usr/bin/false" },
    encoding: "utf8",
    timeout: 30_000,
  });

  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /gosling tui needs an interactive terminal/);
  assert.doesNotMatch(result.stderr, /Raw mode is not supported/);
  assert.equal(result.stdout, "");
});
