import assert from "node:assert/strict";
import { test } from "node:test";
import {
  installGooseExtAgentRequestDispatcher,
  installGooseExtNotificationDispatcher,
} from "../src/generated/client.gen.ts";
import type { GooseSessionNotification_unstable } from "../src/generated/types.gen.ts";
import type {
  RequestPermissionRequest,
  RequestPermissionResponse,
  SessionNotification,
} from "@agentclientprotocol/sdk";

class ClassBackedCallbacks {
  #events: string[] = [];

  get events(): string[] {
    return this.#events;
  }

  async requestPermission(
    _params: RequestPermissionRequest,
  ): Promise<RequestPermissionResponse> {
    this.#events.push("requestPermission");
    return { outcome: { outcome: "cancelled" } };
  }

  async sessionUpdate(_params: SessionNotification): Promise<void> {
    this.#events.push("sessionUpdate");
  }

  async extNotification(
    method: string,
    _params: Record<string, unknown>,
  ): Promise<void> {
    this.#events.push(`extNotification:${method}`);
  }

  async unstable_sessionUpdate(
    notification: GooseSessionNotification_unstable,
  ): Promise<void> {
    this.#events.push(
      `unstable_sessionUpdate:${notification.update.sessionUpdate}`,
    );
  }
}

class MinimalCallbacks {
  async requestPermission(
    _params: RequestPermissionRequest,
  ): Promise<RequestPermissionResponse> {
    return { outcome: { outcome: "cancelled" } };
  }

  async sessionUpdate(_params: SessionNotification): Promise<void> {}
}

class GenericAgentRequestCallbacks extends MinimalCallbacks {
  events: string[] = [];

  async extMethod(
    method: string,
    _params: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    this.events.push(`extMethod:${method}`);
    return { action: "cancel" };
  }
}

const agentRequestParams: Record<string, unknown> = {
  sessionId: "session-1",
};

test("dispatcher preserves class-backed callback receivers", async () => {
  const callbacks = new ClassBackedCallbacks();
  const client = installGooseExtNotificationDispatcher(callbacks);

  await client.requestPermission({} as RequestPermissionRequest);
  await client.sessionUpdate({} as SessionNotification);
  await client.extNotification!("_goose/unstable/session/update", {
    sessionId: "session-1",
    update: {
      sessionUpdate: "status_message",
      status: {
        type: "notice",
        message: "ready",
      },
    },
  });
  await client.extNotification!("example/unknown", {});

  assert.deepEqual(callbacks.events, [
    "requestPermission",
    "sessionUpdate",
    "unstable_sessionUpdate:status_message",
    "extNotification:example/unknown",
  ]);
});

test("raw extNotification is optional", async () => {
  const client = installGooseExtNotificationDispatcher(new MinimalCallbacks());

  await client.extNotification!("example/unknown", {});
});

test("agent request dispatcher falls back to raw extMethod", async () => {
  const callbacks = new GenericAgentRequestCallbacks();
  const client = installGooseExtAgentRequestDispatcher(callbacks);

  const response = await client.extMethod!(
    "_goose/unstable/example/request",
    agentRequestParams,
  );

  assert.deepEqual(response, { action: "cancel" });
  assert.deepEqual(callbacks.events, [
    "extMethod:_goose/unstable/example/request",
  ]);
});

test("agent request dispatcher throws when a request is unhandled", async () => {
  const client = installGooseExtAgentRequestDispatcher(new MinimalCallbacks());

  await assert.rejects(
    () =>
      client.extMethod!("_goose/unstable/example/request", agentRequestParams),
    /unhandled ext method: _goose\/unstable\/example\/request/,
  );
});
