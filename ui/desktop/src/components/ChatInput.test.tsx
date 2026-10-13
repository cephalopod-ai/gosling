import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { ChatState } from '../types/chatState';
import { TooltipProvider } from './ui/Tooltip';
import ChatInput from './ChatInput';

vi.mock('../utils/conversionUtils', () => ({
  compressImageDataUrl: vi.fn(async (data: string) => data),
}));

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
const audio = vi.hoisted(() => ({
  isRecording: false,
  isTranscribing: false,
  onTranscription: undefined as ((text: string) => void) | undefined,
}));

vi.mock('../hooks/useAudioRecorder', () => ({
  useAudioRecorder: (options: { onTranscription: (text: string) => void }) => {
    audio.onTranscription = options.onTranscription;
    return {
      isEnabled: false,
      dictationProvider: null,
      ...audio,
      startRecording: vi.fn(),
      stopRecording: vi.fn(),
    };
  },
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

describe('ChatInput submit guards', () => {
  beforeEach(() => {
    audio.isRecording = false;
    audio.isTranscribing = false;
    vi.stubGlobal(
      'ResizeObserver',
      class {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    );
  });
  afterEach(() => {
    audio.isRecording = false;
    audio.isTranscribing = false;
    vi.unstubAllGlobals();
  });

  it('submits completed dictation from an empty composer after transcription ends', async () => {
    const handleSubmit = vi.fn();
    const input = () => (
      <TooltipProvider>
        <ChatInput
          sessionId="voice"
          handleSubmit={handleSubmit}
          chatState={ChatState.Idle}
          setView={vi.fn()}
        />
      </TooltipProvider>
    );
    audio.isTranscribing = true;
    const { rerender } = render(input(), { wrapper: IntlTestWrapper });
    act(() => audio.onTranscription!('voice draft submit'));
    audio.isTranscribing = false;
    rerender(input());
    await act(async () => new Promise((resolve) => setTimeout(resolve, 150)));
    expect(handleSubmit).toHaveBeenCalledWith({ msg: 'voice draft', images: [] });
  });

  it('still queues complete input while streaming and sends it after the turn', async () => {
    const handleSubmit = vi.fn();
    const input = (state: ChatState) => (
      <TooltipProvider>
        <ChatInput
          sessionId="queue"
          handleSubmit={handleSubmit}
          chatState={state}
          setView={vi.fn()}
          initialPrompt="queue this"
        />
      </TooltipProvider>
    );
    const { rerender } = render(input(ChatState.Streaming), { wrapper: IntlTestWrapper });
    const textbox = screen.getByRole('textbox');
    fireEvent.submit(textbox.closest('form')!);
    expect(handleSubmit).not.toHaveBeenCalled();
    expect(textbox).toHaveValue('');
    rerender(input(ChatState.Idle));
    await act(async () => {});
    expect(handleSubmit).toHaveBeenCalledWith({ msg: 'queue this', images: [] });
  });

  it.each(['recording', 'transcribing', 'restarting', 'image', 'file'])(
    'preserves the draft across keyboard and form submission while %s is pending',
    async (pending) => {
      audio.isRecording = pending === 'recording';
      audio.isTranscribing = pending === 'transcribing';
      const handleSubmit = vi.fn();
      const imageReader = {
        onload: undefined as ((event: { target: { result: string } }) => Promise<void>) | undefined,
        readAsDataURL: vi.fn(),
      };
      if (pending === 'image') {
        vi.stubGlobal(
          'FileReader',
          class {
            constructor() {
              return imageReader;
            }
          }
        );
      }
      render(
        <TooltipProvider>
          <ChatInput
            sessionId="guard"
            handleSubmit={handleSubmit}
            chatState={pending === 'restarting' ? ChatState.RestartingAgent : ChatState.Idle}
            setView={vi.fn()}
            droppedFiles={
              pending === 'file'
                ? [
                    {
                      id: 'loading',
                      name: 'file.txt',
                      path: '/fixture/file.txt',
                      type: 'text/plain',
                      isImage: false,
                      isLoading: true,
                    },
                  ]
                : []
            }
            initialPrompt="keep this draft"
          />
        </TooltipProvider>,
        { wrapper: IntlTestWrapper }
      );
      const textbox = screen.getByRole('textbox');
      if (pending === 'image') {
        fireEvent.paste(textbox, {
          clipboardData: {
            files: [new File(['image'], 'image.png', { type: 'image/png' })],
            getData: () => '',
          },
        });
      }
      for (let count = 0; count < 3; count++) fireEvent.keyDown(textbox, { key: 'Enter' });
      fireEvent.keyDown(textbox, { key: 'Enter', ctrlKey: true, metaKey: true });
      fireEvent.submit(textbox.closest('form')!);
      await act(async () => {});
      expect(handleSubmit).not.toHaveBeenCalled();
      expect(textbox).toHaveValue('keep this draft');
      if (pending === 'image') {
        await act(async () => {
          await imageReader!.onload!({ target: { result: 'data:image/png;base64,aW1hZ2U=' } });
        });
        expect(screen.getByRole('button', { name: 'Remove image' })).toBeInTheDocument();
        fireEvent.submit(textbox.closest('form')!);
        await act(async () => {});
        expect(handleSubmit).toHaveBeenCalledWith({
          msg: 'keep this draft',
          images: [{ data: 'aW1hZ2U=', mimeType: 'image/png' }],
        });
      }
    }
  );
});
