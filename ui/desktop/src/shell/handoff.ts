import type { ShellHandoffEnvelope } from '@repo-makeover/gosling-sdk';
import type { ShellRuntimeIdentity } from './compatibility';

const HANDOFF_SCHEME = 'gosling:';
const HANDOFF_HOST = 'handoff';
const MAX_HANDOFF_URI_BYTES = 64 * 1024;

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function hasExactKeys(
  value: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[] = []
): boolean {
  const allowed = new Set([...required, ...optional]);
  return (
    required.every((key) => Object.prototype.hasOwnProperty.call(value, key)) &&
    Object.keys(value).every((key) => allowed.has(key))
  );
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}

function isOptionalString(value: unknown): value is string | null | undefined {
  return value === undefined || value === null || typeof value === 'string';
}

function isShellHandoffEnvelope(value: unknown): value is ShellHandoffEnvelope {
  if (
    !isRecord(value) ||
    !hasExactKeys(
      value,
      [
        'schemaVersion',
        'handoffId',
        'origin',
        'sourceSessionId',
        'question',
        'requestedCapability',
      ],
      ['references', 'returnDestination', 'allowMutation']
    ) ||
    value.schemaVersion !== 1 ||
    !isNonEmptyString(value.handoffId) ||
    !isNonEmptyString(value.sourceSessionId) ||
    typeof value.question !== 'string' ||
    !isNonEmptyString(value.requestedCapability) ||
    !isOptionalString(value.returnDestination) ||
    (value.allowMutation !== undefined && typeof value.allowMutation !== 'boolean')
  ) {
    return false;
  }

  if (
    !isRecord(value.origin) ||
    !hasExactKeys(value.origin, ['id', 'displayName', 'version']) ||
    !isNonEmptyString(value.origin.id) ||
    !isNonEmptyString(value.origin.displayName) ||
    !isNonEmptyString(value.origin.version)
  ) {
    return false;
  }

  if (value.references === undefined) {
    return true;
  }
  if (!Array.isArray(value.references)) {
    return false;
  }
  return value.references.every(
    (reference) =>
      isRecord(reference) &&
      hasExactKeys(reference, ['kind', 'id'], ['uri']) &&
      isNonEmptyString(reference.kind) &&
      isNonEmptyString(reference.id) &&
      isOptionalString(reference.uri)
  );
}

function encodeEnvelope(envelope: ShellHandoffEnvelope): string {
  if (!isShellHandoffEnvelope(envelope)) {
    throw new Error('handoff envelope is invalid');
  }
  return Buffer.from(JSON.stringify(envelope), 'utf8').toString('base64url');
}

function decodeEnvelope(value: string): ShellHandoffEnvelope | null {
  if (!/^[A-Za-z0-9_-]+$/.test(value)) {
    return null;
  }
  const bytes = Buffer.from(value, 'base64url');
  if (bytes.toString('base64url') !== value) {
    return null;
  }
  const decoded = bytes.toString('utf8');
  if (!Buffer.from(decoded, 'utf8').equals(bytes)) {
    return null;
  }
  try {
    const envelope: unknown = JSON.parse(decoded);
    return isShellHandoffEnvelope(envelope) ? envelope : null;
  } catch {
    return null;
  }
}

export function buildShellHandoffUri(envelope: ShellHandoffEnvelope): string {
  const url = new URL(`${HANDOFF_SCHEME}//${HANDOFF_HOST}`);
  url.searchParams.set('envelope', encodeEnvelope(envelope));
  const result = url.toString();
  if (Buffer.byteLength(result, 'utf8') > MAX_HANDOFF_URI_BYTES) {
    throw new Error('handoff URI exceeds the size limit');
  }
  return result;
}

export function parseShellHandoffUri(value: string): ShellHandoffEnvelope | null {
  if (typeof value !== 'string' || Buffer.byteLength(value, 'utf8') > MAX_HANDOFF_URI_BYTES) {
    return null;
  }
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    return null;
  }
  const parameters = [...parsed.searchParams.entries()];
  if (
    parsed.protocol !== HANDOFF_SCHEME ||
    parsed.host !== HANDOFF_HOST ||
    parsed.pathname !== '' ||
    parsed.username ||
    parsed.password ||
    parsed.hash ||
    parameters.length !== 1 ||
    parameters[0][0] !== 'envelope'
  ) {
    return null;
  }
  return decodeEnvelope(parameters[0][1]);
}

export class ShellHandoffStore {
  private prepared: { generation: number; envelope: ShellHandoffEnvelope } | null = null;

  constructor(
    private readonly identity: ShellRuntimeIdentity,
    private readonly schemaVersion: number
  ) {}

  prepare(generation: number, envelope: ShellHandoffEnvelope): ShellHandoffEnvelope {
    if (
      envelope.schemaVersion !== this.schemaVersion ||
      envelope.origin.id !== this.identity.id ||
      envelope.origin.displayName !== this.identity.displayName ||
      envelope.origin.version !== this.identity.version
    ) {
      throw new Error('server-prepared handoff envelope is incompatible');
    }
    this.prepared = { generation, envelope };
    return envelope;
  }

  confirm(generation: number, handoffId: string): string {
    if (
      !this.prepared ||
      this.prepared.generation !== generation ||
      this.prepared.envelope.handoffId !== handoffId
    ) {
      throw new Error('handoff confirmation is stale or unknown');
    }
    const uri = buildShellHandoffUri(this.prepared.envelope);
    this.prepared = null;
    return uri;
  }

  clear(): void {
    this.prepared = null;
  }
}
