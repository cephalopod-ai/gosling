import { describe, expect, it } from 'vitest';
import {
  classifyShellOperationFailure,
  decodeShellOperationFailure,
  encodeShellOperationFailure,
} from './operationFailure';

describe('shell operation failures', () => {
  it('encodes only stable renderer-facing recovery fields', () => {
    const failure = classifyShellOperationFailure(
      'prompt.submit',
      new Error('transport failed with private backend details')
    );
    expect(failure).toEqual({
      code: 'RUNTIME_UNAVAILABLE',
      message: 'The shell backend is not currently available.',
      retrySafe: true,
      recovery: 'retry',
      preservesDraft: true,
    });
    const encoded = encodeShellOperationFailure(failure);
    expect(encoded).not.toContain('private backend details');
    expect(decodeShellOperationFailure(new Error(encoded))).toEqual(failure);
  });

  it('does not mistake arbitrary errors for trusted failure envelopes', () => {
    expect(decodeShellOperationFailure(new Error('backend said no'))).toBeNull();
    expect(decodeShellOperationFailure('GOSLING_SHELL_FAILURE:{"code":1}')).toBeNull();
  });

  it('projects cross-directory resume rejection as a safe session recovery', () => {
    expect(
      classifyShellOperationFailure(
        'session.resume',
        new Error('session working directory does not match the selected directory')
      )
    ).toEqual({
      code: 'SESSION_UNAVAILABLE',
      message: 'The requested session is not available.',
      retrySafe: false,
      recovery: 'review_session',
      preservesDraft: false,
    });
  });
});
