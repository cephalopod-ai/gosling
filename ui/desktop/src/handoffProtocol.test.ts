import { describe, expect, it, vi } from 'vitest';
import { buildShellHandoffUri } from './shell/handoff';
import {
  dispatchFullGoslingProtocolUrl,
  findGoslingProtocolUrl,
  formatShellHandoffDraft,
  parseGoslingProtocolRoute,
} from './handoffProtocol';

const envelope = {
  schemaVersion: 1,
  handoffId: 'handoff-1',
  origin: { id: 'fixture', displayName: 'Fixture', version: '0.0.0-test' },
  sourceSessionId: 'session-1',
  question: 'Continue this question',
  requestedCapability: 'general_workspace',
  references: [
    { kind: 'artifact', id: 'artifact-1', uri: 'https://example.test/a' },
    { kind: 'memory', id: 'memory-1' },
  ],
  returnDestination: 'fixture://return',
  allowMutation: false,
};

describe('full Gosling handoff protocol router', () => {
  it('extracts the first canonical protocol URL from platform arguments', () => {
    const handoff = buildShellHandoffUri(envelope);
    expect(findGoslingProtocolUrl(['/Applications/Gosling', '--flag', handoff])).toBe(handoff);
    expect(findGoslingProtocolUrl(['/Applications/Gosling', '--flag'])).toBeNull();
    expect(findGoslingProtocolUrl(['GOSLING://handoff?envelope=value'])).toBeNull();
  });

  it('routes a validated handoff without losing the server-prepared envelope', () => {
    const route = parseGoslingProtocolRoute(buildShellHandoffUri(envelope));
    expect(route).toEqual({ action: 'handoff', envelope });
  });

  it('preserves supported legacy protocol actions in the focused router', () => {
    expect(parseGoslingProtocolRoute('gosling://new-session?prompt=Review%20this')).toEqual({
      action: 'new-session',
      prompt: 'Review this',
    });
    expect(parseGoslingProtocolRoute('gosling://resume/session%201')).toEqual({
      action: 'resume',
      sessionId: 'session 1',
    });
    expect(parseGoslingProtocolRoute('gosling://extension?id=one')).toEqual({
      action: 'renderer',
      kind: 'extension',
    });
    expect(parseGoslingProtocolRoute('gosling://sessions/nostr?nevent=one')).toEqual({
      action: 'renderer',
      kind: 'sessions',
    });
  });

  it.each([
    'https://handoff?envelope=value',
    'gosling://unknown',
    'gosling://handoff?envelope=value',
    'gosling://resume',
    'gosling://resume/%E0%A4%A',
  ])('rejects unsupported or malformed routes %s', (value) => {
    expect(parseGoslingProtocolRoute(value)).toBeNull();
  });

  it('dispatches a handoff only as a non-auto-submitted review draft', async () => {
    const operations = {
      openChat: vi.fn(),
      resume: vi.fn(),
      renderer: vi.fn(),
    };
    await expect(
      dispatchFullGoslingProtocolUrl(buildShellHandoffUri(envelope), operations)
    ).resolves.toBe(true);
    expect(operations.openChat).toHaveBeenCalledWith({
      initialMessage: formatShellHandoffDraft(envelope),
      initialMessageNoAutoSubmit: true,
    });
    expect(operations.resume).not.toHaveBeenCalled();
    expect(operations.renderer).not.toHaveBeenCalled();

    await expect(dispatchFullGoslingProtocolUrl('gosling://unknown', operations)).resolves.toBe(
      false
    );
    expect(operations.openChat).toHaveBeenCalledOnce();
  });

  it('creates a reviewable, non-executing draft with exact intent and references', () => {
    expect(formatShellHandoffDraft(envelope)).toBe(
      [
        'Shell handoff received. Review the exact envelope below before sending.',
        'Receiving this draft does not grant the claimed capability, mutation authority, reference access, or return navigation.',
        '',
        JSON.stringify(envelope, null, 2),
      ].join('\n')
    );
  });
});
