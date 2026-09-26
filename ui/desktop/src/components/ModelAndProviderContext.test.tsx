import React from 'react';
import { act, renderHook } from '@testing-library/react';
import { RequestError } from '@agentclientprotocol/sdk';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { ModelAndProviderProvider, useModelAndProvider } from './ModelAndProviderContext';

const mocks = vi.hoisted(() => ({
  acpSetSessionProviderModel: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock('../acp/providers', () => ({
  acpReadDefaults: vi.fn(() => Promise.resolve({ providerId: 'openai', modelId: 'gpt-5' })),
  acpRecordSessionModelSwitch: vi.fn(),
  acpSaveDefaults: vi.fn(),
  acpSetSessionProviderModel: mocks.acpSetSessionProviderModel,
}));

vi.mock('../toasts', () => ({
  toastError: mocks.toastError,
  toastSuccess: vi.fn(),
}));

function wrapper({ children }: { children: React.ReactNode }) {
  return (
    <IntlTestWrapper>
      <ModelAndProviderProvider>{children}</ModelAndProviderProvider>
    </IntlTestWrapper>
  );
}

describe('ModelAndProviderProvider.changeModel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows the server-side cause of a failed session switch', async () => {
    mocks.acpSetSessionProviderModel.mockRejectedValue(
      RequestError.internalError(
        'Provider transition failed: Checkpoint acknowledgement timed out after 120 seconds'
      )
    );
    const { result } = renderHook(() => useModelAndProvider(), { wrapper });

    let switched: boolean | undefined;
    await act(async () => {
      switched = await result.current.changeModel('session-1', {
        name: 'claude-fable-5-1',
        provider: 'claude-code',
      });
    });

    expect(switched).toBe(false);
    expect(mocks.toastError).toHaveBeenCalledWith(
      expect.objectContaining({
        msg: expect.stringContaining(
          'Internal error: Provider transition failed: Checkpoint acknowledgement timed out after 120 seconds'
        ),
      })
    );
    expect(mocks.toastError.mock.calls[0][0].msg).toContain(
      'The switch was not activated. Your previous provider remains active.'
    );
  });
});
