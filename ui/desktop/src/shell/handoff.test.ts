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
    'gosling://user:pass@handoff?envelope=value',
    `gosling://handoff?envelope=${'x'.repeat(65 * 1024)}`,
  ])('rejects malformed or oversized URI %s', (uri) => {
    expect(parseShellHandoffUri(uri)).toBeNull();
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
