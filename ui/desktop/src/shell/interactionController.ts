import { randomUUID } from 'node:crypto';
import type {
  CreateElicitationRequest,
  CreateElicitationResponse,
  ElicitationPropertySchema,
  ElicitationSchema,
  RequestPermissionRequest,
  RequestPermissionResponse,
} from '@agentclientprotocol/sdk';

const MAX_ELICITATION_BYTES = 8 * 1024;
const MAX_ELICITATION_FIELDS = 32;
const MAX_ELICITATION_OPTIONS = 64;
const MAX_ISSUED_ACTION_IDS = 4096;
const SECRET_SHAPED =
  /(api.?key|authorization|cookie|credential|password|private.?key|secret|token)/i;

export interface ShellElicitationOption {
  value: string;
  label: string;
}

export interface ShellElicitationField {
  name: string;
  label: string;
  description: string | null;
  required: boolean;
  type: 'string' | 'number' | 'integer' | 'boolean' | 'multi_select';
  defaultValue?: string | number | boolean | string[];
  options?: ShellElicitationOption[];
  format?: 'email' | 'uri' | 'date' | 'date-time';
  minimum?: number;
  maximum?: number;
  minLength?: number;
  maxLength?: number;
  minItems?: number;
  maxItems?: number;
}

export type ShellInteraction =
  | {
      actionId: string;
      generation: number;
      expiresAtGeneration: number;
      sessionId: string;
      promptAttemptId: string | null;
      kind: 'permission';
      summary: {
        toolTitle: string | null;
        effect: 'read' | 'write' | 'execute' | 'network' | 'other';
        targets: string[];
        inputFields: string[];
        allowOnce: boolean;
        deny: boolean;
      };
    }
  | {
      actionId: string;
      generation: number;
      expiresAtGeneration: number;
      sessionId: string;
      promptAttemptId: string | null;
      kind: 'elicitation';
      summary: {
        message: string;
        title: string | null;
        description: string | null;
        toolCallId: string | null;
        fields: ShellElicitationField[];
      };
    }
  | {
      actionId: string;
      generation: number;
      expiresAtGeneration: number;
      sessionId: string;
      promptAttemptId: string | null;
      kind: 'confirm';
      summary: { action: string; inputFields: string[] };
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
  schema: ElicitationSchema;
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
    action: 'submit' | 'decline' | 'cancel';
    fields?: Record<string, unknown>;
  }): void;
  requestConfirmation(input: {
    actionId: string;
    generation: number;
    sessionId: string;
    action: string;
    actionInput?: unknown;
  }): ShellInteraction;
  respondConfirmation(input: { actionId: string; generation: number; sessionId: string }): void;
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

function boundedOptionalText(value: unknown, maximum: number): string | null {
  const text = boundedText(value, maximum);
  return text.length > 0 ? text : null;
}

function safeFieldNames(value: unknown): string[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return [];
  return Object.keys(value)
    .filter((key) => key.length > 0 && key.length <= 128 && !SECRET_SHAPED.test(key))
    .sort()
    .slice(0, 32);
}

function permissionEffect(kind: unknown): 'read' | 'write' | 'execute' | 'network' | 'other' {
  if (kind === 'read' || kind === 'search') return 'read';
  if (kind === 'edit' || kind === 'delete' || kind === 'move') return 'write';
  if (kind === 'execute') return 'execute';
  if (kind === 'fetch') return 'network';
  return 'other';
}

function targetLabels(request: RequestPermissionRequest): string[] {
  return (request.toolCall.locations ?? [])
    .flatMap((location) => {
      const locationPath = boundedText(location.path, 2048);
      if (!locationPath) return [];
      const label = locationPath.split(/[\\/]/).filter(Boolean).pop() ?? locationPath;
      return [boundedText(label, 256)];
    })
    .filter(Boolean)
    .slice(0, 8);
}

function option(value: string, label = value): ShellElicitationOption | null {
  const safeValue = boundedText(value, 256);
  const safeLabel = boundedText(label, 256);
  return safeValue && safeLabel ? { value: safeValue, label: safeLabel } : null;
}

function propertyOptions(property: ElicitationPropertySchema): ShellElicitationOption[] | null {
  if (property.type === 'string') {
    if (property.enum) {
      if (property.enum.length > MAX_ELICITATION_OPTIONS) return null;
      const options = property.enum.map((value) => option(value));
      return options.every(Boolean) ? (options as ShellElicitationOption[]) : null;
    }
    if (property.oneOf) {
      if (property.oneOf.length > MAX_ELICITATION_OPTIONS) return null;
      const options = property.oneOf.map((entry) => option(entry.const, entry.title));
      return options.every(Boolean) ? (options as ShellElicitationOption[]) : null;
    }
    return [];
  }
  if (property.type !== 'array') return [];
  if ('enum' in property.items) {
    if (property.items.enum.length > MAX_ELICITATION_OPTIONS) return null;
    const options = property.items.enum.map((value) => option(value));
    return options.every(Boolean) ? (options as ShellElicitationOption[]) : null;
  }
  if (property.items.anyOf.length > MAX_ELICITATION_OPTIONS) return null;
  const options = property.items.anyOf.map((entry) => option(entry.const, entry.title));
  return options.every(Boolean) ? (options as ShellElicitationOption[]) : null;
}

function projectElicitationField(
  name: string,
  property: ElicitationPropertySchema,
  required: boolean
): ShellElicitationField | null {
  if (
    !name ||
    name.length > 128 ||
    SECRET_SHAPED.test(name) ||
    ('pattern' in property && property.pattern)
  ) {
    return null;
  }
  const options = propertyOptions(property);
  if (options === null) return null;
  const field: ShellElicitationField = {
    name,
    label: boundedText(property.title, 256) || name,
    description: boundedOptionalText(property.description, 1024),
    required,
    type: property.type === 'array' ? 'multi_select' : property.type,
  };
  if (options.length > 0) field.options = options;
  if (property.type === 'string') {
    if (property.format) field.format = property.format;
    if (property.minLength !== undefined && property.minLength !== null) {
      field.minLength = property.minLength;
    }
    if (property.maxLength !== undefined && property.maxLength !== null) {
      field.maxLength = property.maxLength;
    }
  }
  if (property.type === 'number' || property.type === 'integer') {
    if (property.minimum !== undefined && property.minimum !== null)
      field.minimum = property.minimum;
    if (property.maximum !== undefined && property.maximum !== null)
      field.maximum = property.maximum;
  }
  if (property.type === 'array') {
    if (property.minItems !== undefined && property.minItems !== null)
      field.minItems = property.minItems;
    if (property.maxItems !== undefined && property.maxItems !== null)
      field.maxItems = property.maxItems;
  }
  return field;
}

function projectElicitationSchema(schema: ElicitationSchema): ShellElicitationField[] | null {
  const properties = Object.entries(schema.properties ?? {});
  if (properties.length === 0 || properties.length > MAX_ELICITATION_FIELDS) return null;
  const required = new Set(schema.required ?? []);
  const fields = properties
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, property]) => projectElicitationField(name, property, required.has(name)));
  return fields.every(Boolean) ? (fields as ShellElicitationField[]) : null;
}

function validFormat(value: string, format: string | null | undefined): boolean {
  if (!format) return true;
  if (format === 'email') return /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(value);
  if (format === 'uri') {
    try {
      new URL(value);
      return true;
    } catch {
      return false;
    }
  }
  if (format === 'date') return /^\d{4}-\d{2}-\d{2}$/.test(value);
  return !Number.isNaN(Date.parse(value));
}

function validateFieldValue(value: unknown, property: ElicitationPropertySchema): boolean {
  if (property.type === 'string') {
    if (typeof value !== 'string') return false;
    if (
      property.minLength !== undefined &&
      property.minLength !== null &&
      value.length < property.minLength
    )
      return false;
    if (
      property.maxLength !== undefined &&
      property.maxLength !== null &&
      value.length > property.maxLength
    )
      return false;
    if (!validFormat(value, property.format)) return false;
    const allowed = property.enum ?? property.oneOf?.map((entry) => entry.const);
    return !allowed || allowed.includes(value);
  }
  if (property.type === 'boolean') return typeof value === 'boolean';
  if (property.type === 'number' || property.type === 'integer') {
    if (typeof value !== 'number' || !Number.isFinite(value)) return false;
    if (property.type === 'integer' && !Number.isInteger(value)) return false;
    if (property.minimum !== undefined && property.minimum !== null && value < property.minimum)
      return false;
    return property.maximum === undefined || property.maximum === null || value <= property.maximum;
  }
  if (!Array.isArray(value) || value.some((entry) => typeof entry !== 'string')) return false;
  if (
    property.minItems !== undefined &&
    property.minItems !== null &&
    value.length < property.minItems
  )
    return false;
  if (
    property.maxItems !== undefined &&
    property.maxItems !== null &&
    value.length > property.maxItems
  )
    return false;
  const allowed =
    'enum' in property.items
      ? property.items.enum
      : property.items.anyOf.map((entry) => entry.const);
  return value.every((entry) => allowed.includes(entry));
}

function validateElicitationFields(
  schema: ElicitationSchema,
  fields: Record<string, unknown>
): void {
  const properties = schema.properties ?? {};
  if (Object.keys(fields).some((name) => !Object.prototype.hasOwnProperty.call(properties, name))) {
    throw new Error('elicitation response contains an unknown field');
  }
  for (const name of schema.required ?? []) {
    if (!Object.prototype.hasOwnProperty.call(fields, name))
      throw new Error('elicitation response omits a required field');
  }
  for (const [name, value] of Object.entries(fields)) {
    const property = properties[name];
    if (!property || !validateFieldValue(value, property)) {
      throw new Error('elicitation response contains an invalid field value');
    }
  }
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
    return {
      ...interaction,
      summary: {
        ...interaction.summary,
        targets: [...interaction.summary.targets],
        inputFields: [...interaction.summary.inputFields],
      },
    };
  }
  if (interaction.kind === 'confirm') {
    return {
      ...interaction,
      summary: { ...interaction.summary, inputFields: [...interaction.summary.inputFields] },
    };
  }
  return {
    ...interaction,
    summary: {
      ...interaction.summary,
      fields: interaction.summary.fields.map((field) => ({
        ...field,
        ...(field.options ? { options: field.options.map((entry) => ({ ...entry })) } : {}),
        ...(Array.isArray(field.defaultValue) ? { defaultValue: [...field.defaultValue] } : {}),
      })),
    },
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
      issuedActionIds.size >= MAX_ISSUED_ACTION_IDS ||
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
            effect: permissionEffect(request.toolCall.kind),
            targets: targetLabels(request),
            inputFields: safeFieldNames(request.toolCall.rawInput),
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
      const fields = projectElicitationSchema(request.requestedSchema);
      const message = boundedText(request.message, 2048);
      if (!fields || !message) return Promise.resolve(cancelledElicitation());
      const id = actionId();
      return new Promise<CreateElicitationResponse>((resolve) => {
        const interaction: ShellInteraction = {
          actionId: id,
          generation: input.generation(),
          expiresAtGeneration: input.generation(),
          sessionId: request.sessionId,
          promptAttemptId: promptAttemptId(),
          kind: 'elicitation',
          summary: {
            message,
            title: null,
            description: null,
            toolCallId: boundedOptionalText(request.toolCallId, 512),
            fields,
          },
        };
        pending.set(id, {
          kind: 'elicitation',
          generation: interaction.generation,
          sessionId: request.sessionId,
          schema: request.requestedSchema,
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
      if (action === 'cancel' || action === 'decline') {
        pending.delete(id);
        records.delete(id);
        publishState();
        entry.resolve({ action });
        return;
      }
      const content = assertFields(fields);
      validateElicitationFields(entry.schema, content);
      pending.delete(id);
      records.delete(id);
      publishState();
      entry.resolve({ action: 'accept', content: content as never });
    },
    requestConfirmation({ actionId: id, generation, sessionId, action, actionInput }) {
      if (
        generation !== input.generation() ||
        typeof id !== 'string' ||
        id.length === 0 ||
        id.length > 512 ||
        records.has(id) ||
        issuedActionIds.size >= MAX_ISSUED_ACTION_IDS ||
        issuedActionIds.has(id) ||
        typeof sessionId !== 'string' ||
        sessionId.length === 0 ||
        sessionId.length > 512
      ) {
        throw new Error('domain confirmation is invalid or stale');
      }
      issuedActionIds.add(id);
      const interaction: ShellInteraction = {
        actionId: id,
        generation,
        expiresAtGeneration: generation,
        sessionId,
        promptAttemptId: promptAttemptId(),
        kind: 'confirm',
        summary: {
          action: boundedText(action, 512),
          inputFields: safeFieldNames(actionInput),
        },
      };
      records.set(id, interaction);
      publishState();
      publish(interaction);
      return copyInteraction(interaction);
    },
    respondConfirmation({ actionId: id, generation, sessionId }) {
      const interaction = records.get(id);
      if (
        !interaction ||
        interaction.kind !== 'confirm' ||
        generation !== input.generation() ||
        generation !== interaction.generation ||
        sessionId !== interaction.sessionId
      ) {
        throw new Error('confirmation action is stale');
      }
      records.delete(id);
      publishState();
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
      for (const [id, interaction] of records) {
        if (interaction.kind === 'confirm' && interaction.sessionId === sessionId)
          records.delete(id);
      }
      publishState();
    },
    clear() {
      clearMatching(() => true);
      for (const [id, interaction] of records) {
        if (interaction.kind === 'confirm') records.delete(id);
      }
      publishState();
    },
  };
}
