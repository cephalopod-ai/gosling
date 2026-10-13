import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

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

test(
  "text completion and failure reap an ACP child that ignores SIGTERM",
  { skip: process.platform === "win32" },
  () => {
    for (const fail of [false, true]) {
      const directory = fs.mkdtempSync(
        path.join(os.tmpdir(), "gosling-tui-shutdown-"),
      );
      const binary = path.join(directory, "acp-fixture");
      const pidFile = path.join(directory, "pid");
      fs.writeFileSync(
        binary,
        `#!${process.execPath}
const fs = require('node:fs');
process.on('SIGTERM', () => {});
fs.writeFileSync(process.env.GOSLING_TEST_CHILD_PID, String(process.pid));
setInterval(() => {}, 1000);
const lines = require('node:readline').createInterface({ input: process.stdin });
lines.on('line', line => {
  const request = JSON.parse(line);
  if (request.id === undefined) return;
  let result;
  if (request.method === 'initialize') result = { protocolVersion: 1, agentCapabilities: {} };
  if (request.method === 'session/new') result = { sessionId: 'fixture' };
  if (request.method === 'session/prompt') {
    if (${fail}) {
      console.log(JSON.stringify({ jsonrpc: '2.0', id: request.id, error: { code: -32603, message: 'fixture prompt failed' } }));
      return;
    }
    result = { stopReason: 'end_turn' };
  }
  console.log(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }));
});
`,
        { mode: 0o700 },
      );
      let pid: number | undefined;
      try {
        const started = Date.now();
        const result = spawnSync(
          process.execPath,
          ["--import", "tsx", tuiEntry, "--text", "fixture"],
          {
            stdio: ["ignore", "pipe", "pipe"],
            env: {
              ...process.env,
              GOSLING_BINARY: binary,
              GOSLING_TEST_CHILD_PID: pidFile,
            },
            encoding: "utf8",
            timeout: 15_000,
          },
        );
        pid = Number(fs.readFileSync(pidFile, "utf8"));
        assert.equal(result.status, fail ? 1 : 0, result.stderr);
        assert.ok(Date.now() - started < 10_000, "shutdown must be bounded");
        assert.throws(
          () => process.kill(pid!, 0),
          "ACP child survived TUI shutdown",
        );
        if (fail) assert.match(result.stderr, /fixture prompt failed/);
      } finally {
        if (pid) {
          try {
            process.kill(pid, "SIGKILL");
          } catch {}
        }
        fs.rmSync(directory, { recursive: true, force: true });
      }
    }
  },
);
