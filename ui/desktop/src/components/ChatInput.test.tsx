import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { ChatState } from '../types/chatState';
import { TooltipProvider } from './ui/Tooltip';
import ChatInput from './ChatInput';

vi.mock('./bottom_menu/DirSwitcher', () => ({ DirSwitcher: () => null }));
vi.mock('./bottom_menu/GitBranchIndicator', () => ({ GitBranchIndicator: () => null }));
vi.mock('./bottom_menu/ModeSwitcher', () => ({ ModeSwitcher: () => null }));
vi.mock('./settings/models/bottom_bar/ModelsBottomBar', () => ({ default: () => null }));
vi.mock('./bottom_menu/BottomMenuExtensionSelection', () => ({
  BottomMenuExtensionSelection: () => null,
}));
vi.mock('./bottom_menu/CostTracker', () => ({ CostTracker: () => null }));
vi.mock('./bottom_menu/ContextWindowIndicator', () => ({ ContextWindowIndicator: () => null }));
vi.mock('./MentionPopover', () => ({ default: () => null }));
vi.mock('./ModelAndProviderContext', () => ({
  useModelAndProvider: () => ({
    currentModel: null,
    currentProvider: null,
    getCurrentModelAndProvider: vi.fn(async () => ({ model: '', provider: '' })),
    getCurrentModelDisplayName: vi.fn(async () => ''),
    getCurrentProviderDisplayName: vi.fn(async () => ''),
  }),
}));
vi.mock('../acp/providers', () => ({
  acpListProviderDetails: vi.fn(async () => []),
}));
vi.mock('../hooks/useAudioRecorder', () => ({
  useAudioRecorder: () => ({
    isEnabled: false,
    dictationProvider: null,
    isRecording: false,
    isTranscribing: false,
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
  }),
}));

describe('ChatInput attach button', () => {
  afterEach(() => vi.unstubAllGlobals());

  beforeEach(() => {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    );
  });

  it('has an accessible name', () => {
    render(
      <TooltipProvider>
        <ChatInput
          sessionId="session-1"
          handleSubmit={vi.fn()}
          chatState={ChatState.Idle}
          setView={vi.fn()}
        />
      </TooltipProvider>,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.getByRole('button', { name: 'Attach file' })).toBeInTheDocument();
  });

  it('keeps mounted composers distinct and exposes the selector only for the active one', async () => {
    const inputs = (activeSession: string) => (
      <TooltipProvider>
        {['session-1', 'session-2'].map((sessionId) => (
          <ChatInput
            key={sessionId}
            sessionId={sessionId}
            inactive={sessionId !== activeSession}
            initialValue={sessionId}
            initialPrompt={sessionId}
            handleSubmit={vi.fn()}
            chatState={ChatState.Idle}
            setView={vi.fn()}
          />
        ))}
      </TooltipProvider>
    );
    const { rerender } = render(inputs('session-1'), { wrapper: IntlTestWrapper });
    const ids = screen.getAllByRole('textbox').map((input) => input.id);
    expect(new Set(ids).size).toBe(2);
    expect(ids.every(Boolean)).toBe(true);
    expect(screen.getByTestId('chat-input')).toHaveValue('session-1');
    await act(async () => new Promise((resolve) => setTimeout(resolve, 0)));
    expect(screen.getByTestId('chat-input')).toHaveFocus();
    fireEvent.change(screen.getAllByRole('textbox')[1], { target: { value: 'retained draft' } });

    rerender(inputs('session-2'));
    expect(screen.getByTestId('chat-input')).toHaveValue('retained draft');
    expect(screen.getByTestId('chat-input')).toHaveFocus();
    expect(screen.getAllByRole('textbox').map((input) => input.id)).toEqual(ids);
  });
});

describe('ChatInput restored draft', () => {
  afterEach(() => vi.unstubAllGlobals());

  beforeEach(() => {
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    );
  });

  function renderInput(restoredDraft?: string) {
    return (
      <TooltipProvider>
        <ChatInput
          sessionId="session-1"
          handleSubmit={vi.fn()}
          chatState={ChatState.Idle}
          setView={vi.fn()}
          restoredDraft={restoredDraft}
        />
      </TooltipProvider>
    );
  }

  it('puts a message the backend refused back into the composer and keeps it afterwards', () => {
    const { rerender } = render(renderInput(), { wrapper: IntlTestWrapper });
    const textbox = screen.getByRole('textbox') as HTMLTextAreaElement;
    expect(textbox.value).toBe('');

    rerender(renderInput('TAB-B'));
    expect(textbox.value).toBe('TAB-B');

    rerender(renderInput(undefined));
    expect(textbox.value).toBe('TAB-B');
  });
});
