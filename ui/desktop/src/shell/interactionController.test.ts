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
      toolCall: {
        toolCallId: 'tool-a',
        title: 'Tool title',
        kind: 'execute',
        status: 'pending',
        locations: [{ path: '/private/workspace/source.rs', line: 4 }],
        rawInput: { command: 'cargo test', apiToken: 'must-not-project' },
      },
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
        summary: {
          toolTitle: 'Tool title',
          effect: 'execute',
          targets: ['source.rs'],
          inputFields: ['command'],
          allowOnce: true,
          deny: true,
        },
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

  it('projects and validates a bounded elicitation schema without secret-shaped fields', async () => {
    const controller = createShellInteractionController({
      generation: () => 1,
      createActionId: () => 'form',
    });
    const response = controller.requestElicitation({
      mode: 'form',
      sessionId: 'session-a',
      message: 'Choose a mode',
      requestedSchema: {
        properties: {
          count: { type: 'integer', title: 'Count', minimum: 1, maximum: 3 },
          mode: { type: 'string', enum: ['safe', 'fast'], default: 'safe' },
        },
        required: ['count'],
      },
    });
    expect(controller.read()[0]).toMatchObject({
      kind: 'elicitation',
      summary: {
        message: 'Choose a mode',
        fields: [
          {
            name: 'count',
            label: 'Count',
            required: true,
            type: 'integer',
            minimum: 1,
            maximum: 3,
          },
          {
            name: 'mode',
            required: false,
            type: 'string',
            options: [
              { value: 'safe', label: 'safe' },
              { value: 'fast', label: 'fast' },
            ],
          },
        ],
      },
    });
    expect(() =>
      controller.respondElicitation({
        actionId: 'form',
        generation: 1,
        sessionId: 'session-a',
        action: 'submit',
        fields: { count: 9 },
      })
    ).toThrow('invalid field value');
    expect(controller.read()).toHaveLength(1);
    controller.respondElicitation({
      actionId: 'form',
      generation: 1,
      sessionId: 'session-a',
      action: 'decline',
    });
    await expect(response).resolves.toEqual({ action: 'decline' });

    await expect(
      controller.requestElicitation({
        mode: 'form',
        sessionId: 'session-a',
        message: 'Secret',
        requestedSchema: { properties: { apiToken: { type: 'string' } } },
      })
    ).resolves.toEqual({ action: 'cancel' });
  });

  it('mirrors a server-owned domain confirmation as a safe pending fact', () => {
    const controller = createShellInteractionController({ generation: () => 2 });
    const requested: unknown[] = [];
    controller.onRequested((interaction) => requested.push(interaction));
    controller.requestConfirmation({
      actionId: 'confirm-a',
      generation: 2,
      sessionId: 'session-a',
      action: 'replace-output',
      actionInput: { outputId: 'one', credentialToken: 'discarded' },
    });
    expect(requested).toEqual([
      expect.objectContaining({
        actionId: 'confirm-a',
        kind: 'confirm',
        summary: { action: 'replace-output', inputFields: ['outputId'] },
      }),
    ]);
    controller.respondConfirmation({
      actionId: 'confirm-a',
      generation: 2,
      sessionId: 'session-a',
    });
    expect(controller.read()).toEqual([]);
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

  it('fails closed when the process-lifetime replay ledger reaches its bound', () => {
    const controller = createShellInteractionController({ generation: () => 1 });
    for (let index = 0; index < 4096; index += 1) {
      const actionId = `confirm-${index}`;
      controller.requestConfirmation({
        actionId,
        generation: 1,
        sessionId: 'session-a',
        action: 'mutate',
      });
      controller.respondConfirmation({ actionId, generation: 1, sessionId: 'session-a' });
    }

    expect(() =>
      controller.requestConfirmation({
        actionId: 'confirm-overflow',
        generation: 1,
        sessionId: 'session-a',
        action: 'mutate',
      })
    ).toThrow('invalid or stale');
  });
});
