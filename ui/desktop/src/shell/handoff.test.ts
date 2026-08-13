import { describe, expect, it } from 'vitest';
import { buildShellHandoffUri, parseShellHandoffUri, ShellHandoffStore } from './handoff';

const identity = { id: 'fixture', displayName: 'Fixture', version: '0.0.0-test' };
const envelope = {
  schemaVersion: 1,
  handoffId: 'handoff-1',
  origin: identity,
  sourceSessionId: 'session-1',
  question: 'Continue this question',
  requestedCapability: 'general_workspace',
  references: [{ kind: 'artifact', id: 'artifact-1', uri: 'https://example.test/a' }],
  returnDestination: 'fixture://return',
  allowMutation: false,
};

function uriFor(value: unknown): string {
  return `gosling://handoff?envelope=${Buffer.from(JSON.stringify(value), 'utf8').toString('base64url')}`;
}

describe('shell handoff', () => {
  it('round-trips the exact server envelope through the bounded Gosling URI', () => {
    const uri = buildShellHandoffUri(envelope);
    expect(uri.startsWith('gosling://handoff?')).toBe(true);
    expect(parseShellHandoffUri(uri)).toEqual(envelope);
  });

  it.each([
    'https://handoff?envelope=value',
    'gosling://other?envelope=value',
    'gosling://handoff/path?envelope=value',
    'gosling://handoff?other=value',
    'gosling://handoff?envelope=one&envelope=two',
    'gosling://handoff?envelope=value#fragment',
    'gosling://handoff:80?envelope=value',
    'gosling://user:pass@handoff?envelope=value',
    'gosling://HANDOFF?envelope=value',
    'gosling://handoff?envelope=e30=',
    `gosling://handoff?envelope=${'x'.repeat(65 * 1024)}`,
  ])('rejects malformed or oversized URI %s', (uri) => {
    expect(parseShellHandoffUri(uri)).toBeNull();
  });

  it.each([
    { ...envelope, schemaVersion: 2 },
    { ...envelope, extra: true },
    { ...envelope, handoffId: '' },
    { ...envelope, sourceSessionId: '' },
    { ...envelope, question: 42 },
    { ...envelope, requestedCapability: '' },
    { ...envelope, origin: { ...identity, extra: true } },
    { ...envelope, references: {} },
    { ...envelope, references: [{ kind: 'artifact', id: 'one', extra: true }] },
    { ...envelope, references: [{ kind: '', id: 'one' }] },
    { ...envelope, returnDestination: false },
    { ...envelope, allowMutation: 'false' },
  ])('rejects structurally invalid envelopes %#', (value) => {
    expect(parseShellHandoffUri(uriFor(value))).toBeNull();
  });

  it('rejects invalid envelopes before encoding', () => {
    expect(() => buildShellHandoffUri({ ...envelope, question: 42 } as never)).toThrow(
      'handoff envelope is invalid'
    );
  });

  it('confirms only the current server-prepared ID and consumes it once', () => {
    const store = new ShellHandoffStore(identity, 1);
    expect(store.prepare(4, envelope)).toEqual(envelope);
    expect(() => store.confirm(3, envelope.handoffId)).toThrow('stale or unknown');
    expect(() => store.confirm(4, 'other')).toThrow('stale or unknown');
    const uri = store.confirm(4, envelope.handoffId);
    expect(parseShellHandoffUri(uri)).toEqual(envelope);
    expect(() => store.confirm(4, envelope.handoffId)).toThrow('stale or unknown');
  });

  it('rejects incompatible schema or origin before retaining authority', () => {
    const store = new ShellHandoffStore(identity, 1);
    expect(() => store.prepare(1, { ...envelope, schemaVersion: 2 })).toThrow('incompatible');
    expect(() => store.prepare(1, { ...envelope, origin: { ...identity, id: 'other' } })).toThrow(
      'incompatible'
    );
    expect(() => store.confirm(1, envelope.handoffId)).toThrow('stale or unknown');
  });
});
