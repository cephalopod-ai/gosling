import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { acpDebug, redactAcpDebugPayload } from "../src/http-stream.js";

const sentinel = "SENTINEL_WORKSPACE_SECRET";
const originalDebug = console.debug;

afterEach(() => {
  delete (globalThis as { ACP_DEBUG?: unknown }).ACP_DEBUG;
  console.debug = originalDebug;
});

test("redacts credential fields without mutating ordinary ACP data", () => {
  const redacted = redactAcpDebugPayload({
    method: "_gosling/unstable/credential-profiles/create",
    params: {
      name: "AFRL OpenAI",
      secretFields: [{ key: "OPENAI_API_KEY", value: sentinel }],
      nested: { password: sentinel },
    },
    result: { message: `provider rejected api_key=${sentinel}` },
  });
  const encoded = JSON.stringify(redacted);

  assert.equal(encoded.includes(sentinel), false);
  assert.equal(encoded.includes("AFRL OpenAI"), true);
  assert.equal(encoded.includes("[redacted]"), true);
});

test("ACP debug output never logs the original credential value", () => {
  const calls: unknown[][] = [];
  (globalThis as { ACP_DEBUG?: unknown }).ACP_DEBUG = true;
  console.debug = (...args: unknown[]) => calls.push(args);

  acpDebug("POST → agent", {
    params: { secretFields: [{ key: "OPENAI_API_KEY", value: sentinel }] },
  });

  assert.equal(calls.length, 1);
  assert.equal(JSON.stringify(calls).includes(sentinel), false);
});
