import React from 'react';
import { act, renderHook } from '@testing-library/react';
import { RequestError } from '@agentclientprotocol/sdk';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { ModelAndProviderProvider, useModelAndProvider } from './ModelAndProviderContext';
import { acpSaveDefaults } from '../acp/providers';
import { acpChatSessionActions, acpChatSessionStore } from '../acp/chatSessionStore';

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

  it('saves the default context window with the model', async () => {
    const { result } = renderHook(() => useModelAndProvider(), { wrapper });
    await act(async () => {
      expect(
        await result.current.changeModel(null, {
          name: 'spark',
          provider: 'together',
          context_limit: 1_048_576,
        })
      ).toBe(true);
    });
    expect(acpSaveDefaults).toHaveBeenCalledWith('together', 'spark', 1_048_576);
  });

  it('replaces stale session context metadata after a switch', async () => {
    acpChatSessionActions.setSessionMetadata('context-session', {
      id: 'context-session',
      provider_name: 'openai',
      model_config: { model_name: 'gpt-5', toolshim: false, context_limit: 128_000 },
    } as Parameters<typeof acpChatSessionActions.setSessionMetadata>[1]);
    mocks.acpSetSessionProviderModel.mockResolvedValue({
      providerId: 'together',
      modelId: 'spark',
      contextLimit: 1_048_576,
    });
    const { result } = renderHook(() => useModelAndProvider(), { wrapper });
    await act(async () => {
      expect(
        await result.current.changeModel('context-session', {
          name: 'spark',
          provider: 'together',
          context_limit: 1_048_576,
        })
      ).toBe(true);
    });
    expect(
      acpChatSessionStore.getSnapshot('context-session')?.session?.model_config?.context_limit
    ).toBe(1_048_576);
    mocks.acpSetSessionProviderModel.mockResolvedValue({
      providerId: 'together',
      modelId: 'unknown',
    });
    await act(async () => {
      await result.current.changeModel('context-session', {
        name: 'unknown',
        provider: 'together',
      });
    });
    expect(
      acpChatSessionStore.getSnapshot('context-session')?.session?.model_config?.context_limit
    ).toBeNull();
  });
});
