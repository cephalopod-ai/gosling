import { describe, expect, it } from 'vitest';
import { asOperationFailure, resolveShellApi } from './api';
import type { ShellOperationFailure } from '../shell/operationFailure';

const VALID: ShellOperationFailure = {
  code: 'SESSION_BUSY',
  message: 'Finish or stop the current task before continuing.',
  retrySafe: false,
  recovery: 'review_session',
  preservesDraft: true,
};

describe('failure normalisation', () => {
  it('passes a well-formed failure through unchanged', () => {
    expect(asOperationFailure(VALID)).toEqual(VALID);
  });

  it.each([
    ['a raw Error', new Error('ECONNREFUSED 127.0.0.1:52713')],
    ['a bare string', 'backend exploded at /Users/someone/secret/path'],
    ['null', null],
    ['undefined', undefined],
    ['an array', [VALID]],
    ['an unknown code', { ...VALID, code: 'NOT_A_CODE' }],
    ['an unknown recovery action', { ...VALID, recovery: 'sudo' }],
    ['a non-boolean retrySafe', { ...VALID, retrySafe: 'yes' }],
    ['an empty message', { ...VALID, message: '' }],
    ['an oversized message', { ...VALID, message: 'x'.repeat(513) }],
  ])('reduces %s to OPERATION_FAILED', (_label, error) => {
    const failure = asOperationFailure(error);
    expect(failure.code).toBe('OPERATION_FAILED');
    expect(failure.recovery).toBe('save_diagnostics');
    expect(failure.retrySafe).toBe(false);
  });

  it('never surfaces the original error text', () => {
    const failure = asOperationFailure(new Error('/Users/someone/.gosling/secrets.json'));
    expect(failure.message).not.toContain('/Users/someone');
    expect(JSON.stringify(failure)).not.toContain('secrets.json');
  });

  it('drops extra fields rather than forwarding them', () => {
    const failure = asOperationFailure({
      ...VALID,
      serverStack: 'at Object.<anonymous>',
    }) as unknown as Record<string, unknown>;
    expect(Object.keys(failure).sort()).toEqual([
      'code',
      'message',
      'preservesDraft',
      'recovery',
      'retrySafe',
    ]);
  });
});

describe('preload availability', () => {
  it('fails loudly when the preload bridge is absent', () => {
    const original = (globalThis.window as { goslingShell?: unknown }).goslingShell;
    delete (globalThis.window as { goslingShell?: unknown }).goslingShell;
    expect(() => resolveShellApi()).toThrow(/preload API is unavailable/);
    if (original) (globalThis.window as { goslingShell?: unknown }).goslingShell = original;
  });
});
