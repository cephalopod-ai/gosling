import { randomUUID } from 'node:crypto';
import type {
  CreateElicitationRequest,
  CreateElicitationResponse,
  RequestPermissionRequest,
  RequestPermissionResponse,
} from '@agentclientprotocol/sdk';

const MAX_ELICITATION_BYTES = 8 * 1024;

export type ShellInteraction =
  | {
      actionId: string;
      generation: number;
      expiresAtGeneration: number;
      sessionId: string;
      promptAttemptId: string | null;
      kind: 'permission';
      summary: { toolTitle: string | null; allowOnce: boolean; deny: boolean };
    }
  | {
      actionId: string;
      generation: number;
      expiresAtGeneration: number;
      sessionId: string;
      promptAttemptId: string | null;
      kind: 'elicitation';
      summary: { message: string; fields: string[] };
    };

interface PendingPermission {
  kind: 'permission';
  generation: number;
  sessionId: string;
  request: RequestPermissionRequest;
  resolve: (response: RequestPermissionResponse) => void;
}

interface PendingElicitation {
  kind: 'elicitation';
  generation: number;
  sessionId: string;
  resolve: (response: CreateElicitationResponse) => void;
}

export interface ShellInteractionController {
  requestPermission(request: RequestPermissionRequest): Promise<RequestPermissionResponse>;
  requestElicitation(request: CreateElicitationRequest): Promise<CreateElicitationResponse>;
  respondPermission(input: {
    actionId: string;
    generation: number;
    sessionId: string;
    allowOnce: boolean;
  }): void;
  respondElicitation(input: {
    actionId: string;
    generation: number;
    sessionId: string;
    action: 'submit' | 'cancel';
    fields?: Record<string, unknown>;
  }): void;
  onRequested(listener: (interaction: ShellInteraction) => void): () => void;
  onChanged(listener: () => void): () => void;
  read(): ShellInteraction[];
  clearSession(sessionId: string): void;
  clear(): void;
}

function cancelledPermission(): RequestPermissionResponse {
  return { outcome: { outcome: 'cancelled' } };
}

function cancelledElicitation(): CreateElicitationResponse {
  return { action: 'cancel' };
}

function boundedText(value: unknown, maximum: number): string {
  return typeof value === 'string' && value.length <= maximum ? value : '';
}

function assertFields(fields: Record<string, unknown> | undefined): Record<string, unknown> {
  const value = fields ?? {};
  const encoded = JSON.stringify(value);
  if (Buffer.byteLength(encoded, 'utf8') > MAX_ELICITATION_BYTES) {
    throw new Error('elicitation response exceeds the channel size limit');
  }
  return value;
}

function copyInteraction(interaction: ShellInteraction): ShellInteraction {
  if (interaction.kind === 'permission') {
    return { ...interaction, summary: { ...interaction.summary } };
  }
  return {
    ...interaction,
    summary: { ...interaction.summary, fields: [...interaction.summary.fields] },
  };
}

export function createShellInteractionController(input: {
  generation: () => number;
  promptAttemptId?: () => string | null;
  createActionId?: () => string;
}): ShellInteractionController {
  const pending = new Map<string, PendingPermission | PendingElicitation>();
  const records = new Map<string, ShellInteraction>();
  const issuedActionIds = new Set<string>();
  const listeners = new Set<(interaction: ShellInteraction) => void>();
  const stateListeners = new Set<() => void>();
  const createActionId = input.createActionId ?? randomUUID;
  const promptAttemptId = input.promptAttemptId ?? (() => null);

  const publish = (interaction: ShellInteraction) => {
    for (const listener of listeners) listener(interaction);
  };

  const publishState = () => {
    for (const listener of stateListeners) listener();
  };

  const actionId = () => {
    const value = createActionId();
    if (
      typeof value !== 'string' ||
      value.length === 0 ||
      value.length > 512 ||
      issuedActionIds.has(value)
    ) {
      throw new Error('interaction action ID is invalid');
    }
    issuedActionIds.add(value);
    return value;
  };

  const clearMatching = (matches: (entry: PendingPermission | PendingElicitation) => boolean) => {
    for (const [id, entry] of pending) {
      if (!matches(entry)) continue;
      pending.delete(id);
      records.delete(id);
      publishState();
      if (entry.kind === 'permission') entry.resolve(cancelledPermission());
      else entry.resolve(cancelledElicitation());
    }
  };

  return {
    requestPermission(request) {
      if (typeof request.sessionId !== 'string' || request.sessionId.length === 0) {
        return Promise.resolve(cancelledPermission());
      }
      const id = actionId();
      const allowOnce = request.options.some((option) => option.kind === 'allow_once');
      const deny = request.options.some((option) => option.kind === 'reject_once');
      return new Promise<RequestPermissionResponse>((resolve) => {
        const interaction: ShellInteraction = {
          actionId: id,
          generation: input.generation(),
          expiresAtGeneration: input.generation(),
          sessionId: request.sessionId,
          promptAttemptId: promptAttemptId(),
          kind: 'permission',
          summary: {
            toolTitle: boundedText(request.toolCall.title, 512) || null,
            allowOnce,
            deny,
          },
        };
        pending.set(id, {
          kind: 'permission',
          generation: interaction.generation,
          sessionId: request.sessionId,
          request,
          resolve,
        });
        records.set(id, interaction);
        publishState();
        publish(interaction);
      });
    },
    requestElicitation(request) {
      if (
        request.mode !== 'form' ||
        !('sessionId' in request) ||
        typeof request.sessionId !== 'string' ||
        request.sessionId.length === 0
      ) {
        return Promise.resolve(cancelledElicitation());
      }
      const id = actionId();
      const fields = Object.keys(request.requestedSchema.properties ?? {})
        .sort()
        .slice(0, 64);
      return new Promise<CreateElicitationResponse>((resolve) => {
        const interaction: ShellInteraction = {
          actionId: id,
          generation: input.generation(),
          expiresAtGeneration: input.generation(),
          sessionId: request.sessionId,
          promptAttemptId: promptAttemptId(),
          kind: 'elicitation',
          summary: { message: boundedText(request.message, 2048), fields },
        };
        pending.set(id, {
          kind: 'elicitation',
          generation: interaction.generation,
          sessionId: request.sessionId,
          resolve,
        });
        records.set(id, interaction);
        publishState();
        publish(interaction);
      });
    },
    respondPermission({ actionId: id, generation, sessionId, allowOnce }) {
      const entry = pending.get(id);
      if (
        !entry ||
        entry.kind !== 'permission' ||
        generation !== input.generation() ||
        generation !== entry.generation ||
        sessionId !== entry.sessionId
      ) {
        throw new Error('permission action is stale');
      }
      pending.delete(id);
      records.delete(id);
      publishState();
      const kind = allowOnce ? 'allow_once' : 'reject_once';
      const optionId = entry.request.options.find((option) => option.kind === kind)?.optionId;
      entry.resolve(
        optionId ? { outcome: { outcome: 'selected', optionId } } : cancelledPermission()
      );
    },
    respondElicitation({ actionId: id, generation, sessionId, action, fields }) {
      const entry = pending.get(id);
      if (
        !entry ||
        entry.kind !== 'elicitation' ||
        generation !== input.generation() ||
        generation !== entry.generation ||
        sessionId !== entry.sessionId
      ) {
        throw new Error('elicitation action is stale');
      }
      if (action === 'cancel') {
        pending.delete(id);
        records.delete(id);
        publishState();
        entry.resolve(cancelledElicitation());
        return;
      }
      const content = assertFields(fields);
      pending.delete(id);
      records.delete(id);
      publishState();
      entry.resolve({ action: 'accept', content: content as never });
    },
    onRequested(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    onChanged(listener) {
      stateListeners.add(listener);
      return () => stateListeners.delete(listener);
    },
    read: () => [...records.values()].map(copyInteraction),
    clearSession(sessionId) {
      clearMatching((entry) => entry.sessionId === sessionId);
    },
    clear() {
      clearMatching(() => true);
    },
  };
}
