import type { ShellHandoffEnvelope } from '@repo-makeover/gosling-sdk';
import type { ShellRuntimeIdentity } from './compatibility';

const HANDOFF_SCHEME = 'gosling:';
const HANDOFF_HOST = 'handoff';
const MAX_HANDOFF_URI_BYTES = 64 * 1024;

function encodeEnvelope(envelope: ShellHandoffEnvelope): string {
  return Buffer.from(JSON.stringify(envelope), 'utf8').toString('base64url');
}

function decodeEnvelope(value: string): ShellHandoffEnvelope {
  const decoded = Buffer.from(value, 'base64url').toString('utf8');
  return JSON.parse(decoded) as ShellHandoffEnvelope;
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
  if (Buffer.byteLength(value, 'utf8') > MAX_HANDOFF_URI_BYTES) {
    return null;
  }
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    return null;
  }
  if (
    parsed.protocol !== HANDOFF_SCHEME ||
    parsed.hostname !== HANDOFF_HOST ||
    parsed.pathname !== '' ||
    parsed.username ||
    parsed.password ||
    [...parsed.searchParams.keys()].some((key) => key !== 'envelope')
  ) {
    return null;
  }
  const encoded = parsed.searchParams.get('envelope');
  if (!encoded) {
    return null;
  }
  try {
    const envelope = decodeEnvelope(encoded);
    return envelope.schemaVersion === 1 && envelope.handoffId ? envelope : null;
  } catch {
    return null;
  }
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
