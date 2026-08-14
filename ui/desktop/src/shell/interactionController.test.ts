import { describe, expect, it } from 'vitest';
import { createShellInteractionController } from './interactionController';

describe('shell interaction controller', () => {
  it('holds permissions until an explicit single-use response', async () => {
    const controller = createShellInteractionController({
      generation: () => 4,
      createActionId: () => 'permit',
    });
    const requested: unknown[] = [];
    controller.onRequested((interaction) => requested.push(interaction));
    const response = controller.requestPermission({
      sessionId: 'session-a',
      toolCall: { toolCallId: 'tool-a', title: 'Tool title', kind: 'execute', status: 'pending' },
      options: [
        { optionId: 'allow', name: 'Allow once', kind: 'allow_once' },
        { optionId: 'deny', name: 'Deny', kind: 'reject_once' },
      ],
    });
    expect(requested).toEqual([
      expect.objectContaining({
        actionId: 'permit',
        generation: 4,
        kind: 'permission',
        sessionId: 'session-a',
      }),
    ]);
    controller.respondPermission({
      actionId: 'permit',
      generation: 4,
      sessionId: 'session-a',
      allowOnce: true,
    });
    await expect(response).resolves.toEqual({
      outcome: { outcome: 'selected', optionId: 'allow' },
    });
    expect(() =>
      controller.respondPermission({
        actionId: 'permit',
        generation: 4,
        sessionId: 'session-a',
        allowOnce: true,
      })
    ).toThrow('stale');
  });

  it('cancels unsupported elicitation and fences bounded submitted forms', async () => {
    const controller = createShellInteractionController({
      generation: () => 1,
      createActionId: () => 'form',
    });
    await expect(
      controller.requestElicitation({
        mode: 'url',
        requestId: 'request-a',
        elicitationId: 'elicitation-a',
        message: 'Open',
        url: 'https://example.test',
      })
    ).resolves.toEqual({ action: 'cancel' });
    const response = controller.requestElicitation({
      mode: 'form',
      sessionId: 'session-a',
      message: 'Choose',
      requestedSchema: { properties: { choice: { type: 'string' } } },
    });
    controller.respondElicitation({
      actionId: 'form',
      generation: 1,
      sessionId: 'session-a',
      action: 'submit',
      fields: { choice: 'yes' },
    });
    await expect(response).resolves.toMatchObject({ action: 'accept', content: { choice: 'yes' } });
  });

  it('keeps a pending form available after rejecting an oversized response', async () => {
    const controller = createShellInteractionController({
      generation: () => 1,
      createActionId: () => 'form',
    });
    const response = controller.requestElicitation({
      mode: 'form',
      sessionId: 'session-a',
      message: 'Choose',
      requestedSchema: { properties: { choice: { type: 'string' } } },
    });
    expect(() =>
      controller.respondElicitation({
        actionId: 'form',
        generation: 1,
        sessionId: 'session-a',
        action: 'submit',
        fields: { choice: 'x'.repeat(8 * 1024) },
      })
    ).toThrow('channel size limit');
    controller.respondElicitation({
      actionId: 'form',
      generation: 1,
      sessionId: 'session-a',
      action: 'cancel',
    });
    await expect(response).resolves.toEqual({ action: 'cancel' });
  });

  it('rejects an interaction response from a stale generation without consuming it', async () => {
    let generation = 1;
    const controller = createShellInteractionController({
      generation: () => generation,
      createActionId: () => 'permit',
    });
    const response = controller.requestPermission({
      sessionId: 'session-a',
      toolCall: { toolCallId: 'tool-a', title: 'Tool title', kind: 'execute', status: 'pending' },
      options: [{ optionId: 'deny', name: 'Deny', kind: 'reject_once' }],
    });
    generation = 2;

    expect(() =>
      controller.respondPermission({
        actionId: 'permit',
        generation: 1,
        sessionId: 'session-a',
        allowOnce: false,
      })
    ).toThrow('stale');
    expect(controller.read()).toHaveLength(1);

    controller.clear();
    await expect(response).resolves.toEqual({ outcome: { outcome: 'cancelled' } });
  });

  it('rejects a response for another session without consuming the interaction', async () => {
    const controller = createShellInteractionController({
      generation: () => 1,
      createActionId: () => 'permit',
    });
    const response = controller.requestPermission({
      sessionId: 'session-a',
      toolCall: { toolCallId: 'tool-a', title: 'Tool title', kind: 'execute', status: 'pending' },
      options: [{ optionId: 'deny', name: 'Deny', kind: 'reject_once' }],
    });

    expect(() =>
      controller.respondPermission({
        actionId: 'permit',
        generation: 1,
        sessionId: 'session-b',
        allowOnce: false,
      })
    ).toThrow('stale');
    expect(controller.read()).toHaveLength(1);

    controller.respondPermission({
      actionId: 'permit',
      generation: 1,
      sessionId: 'session-a',
      allowOnce: false,
    });
    await expect(response).resolves.toEqual({
      outcome: { outcome: 'selected', optionId: 'deny' },
    });
  });
});
