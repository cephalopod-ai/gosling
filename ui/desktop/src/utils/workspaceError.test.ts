import { RequestError } from '@agentclientprotocol/sdk';
import { describe, expect, it } from 'vitest';
import { workspaceErrorMessage } from './workspaceError';

describe('workspaceErrorMessage', () => {
  it('shows the reason the backend gave for rejecting a request', () => {
    expect(
      workspaceErrorMessage(
        RequestError.invalidParams('workspace name is already in use'),
        'Unable to save workspace'
      )
    ).toBe('workspace name is already in use');
  });

  it('keeps the kind of failure next to the detail of any other request error', () => {
    expect(
      workspaceErrorMessage(
        RequestError.internalError('Failed to list credential profiles: disk full'),
        'Unable to load'
      )
    ).toBe('Internal error: Failed to list credential profiles: disk full');
    expect(workspaceErrorMessage(RequestError.invalidParams(), 'Unable to save')).toBe(
      'Invalid params'
    );
  });

  it('leaves other errors as they were', () => {
    expect(workspaceErrorMessage(new Error('folder is unavailable'), 'Unable to save')).toBe(
      'folder is unavailable'
    );
    expect(workspaceErrorMessage(undefined, 'Unable to save')).toBe('Unable to save');
  });

  it('still redacts secrets carried in the reason', () => {
    expect(
      workspaceErrorMessage(
        RequestError.invalidParams('rejected api_key=sk-live-abcdefghijk'),
        'Unable to save'
      )
    ).toBe('rejected api_key=[redacted]');
  });
});
