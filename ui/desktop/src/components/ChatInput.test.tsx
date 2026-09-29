import { render, screen } from '@testing-library/react';
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
});
