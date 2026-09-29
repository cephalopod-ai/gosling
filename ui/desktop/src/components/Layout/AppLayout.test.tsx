import { act, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { AppLayout } from './AppLayout';

const workbench = vi.hoisted(() => ({ isOpen: true, width: 480, toggle: vi.fn() }));

vi.mock('react-router-dom', () => ({
  Outlet: () => null,
  useLocation: () => ({ pathname: '/' }),
}));
vi.mock('../../contexts/ChatContext', () => ({ useChatContext: () => ({ setChat: vi.fn() }) }));
vi.mock('../../contexts/ArtifactRouterContext', () => ({
  ArtifactRouterProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));
vi.mock('../../contexts/ArtifactWorkbenchContext', () => ({
  ArtifactWorkbenchProvider: ({ children }: { children: React.ReactNode }) => <>{children}</>,
  useArtifactWorkbench: () => workbench,
}));
vi.mock('../artifacts/ArtifactPane', () => ({ ArtifactPane: () => null }));
vi.mock('../ChatSessionsContainer', () => ({ default: () => null }));
vi.mock('./NavigationPanel', () => ({ Navigation: () => null }));

function setWindowWidth(width: number) {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: width });
}

function renderLayout() {
  return render(<AppLayout activeSessions={[]} />, { wrapper: IntlTestWrapper });
}

describe('AppLayout outputs pane', () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.electron = {
      ...window.electron,
      platform: 'darwin',
      getIsFullScreen: vi.fn().mockResolvedValue(false),
      on: vi.fn(),
    } as unknown as typeof window.electron;
  });

  it('overlays the pane instead of crushing the hub at the default 940px window', () => {
    setWindowWidth(940);
    renderLayout();

    const frame = screen.getByTestId('artifact-pane-frame');
    expect(frame).toHaveAttribute('data-pane-mode', 'overlay');
    expect(frame).toHaveClass('absolute', 'right-0');
    expect(frame).not.toHaveClass('relative');
  });

  it('docks the pane in the row once the window is wide enough and follows resizes', () => {
    setWindowWidth(1600);
    renderLayout();
    const frame = screen.getByTestId('artifact-pane-frame');
    expect(frame).toHaveAttribute('data-pane-mode', 'docked');
    expect(frame).toHaveClass('relative');

    act(() => {
      setWindowWidth(900);
      window.dispatchEvent(new Event('resize'));
    });
    expect(screen.getByTestId('artifact-pane-frame')).toHaveAttribute('data-pane-mode', 'overlay');
  });
});
