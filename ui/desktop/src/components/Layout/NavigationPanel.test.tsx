import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { useNavigationContext } from './NavigationContext';
import { useNavigationSessions } from '../../hooks/useNavigationSessions';
import { Navigation } from './NavigationPanel';

vi.mock('./NavigationContext', () => ({
  useNavigationContext: vi.fn(),
}));

vi.mock('../../hooks/useNavigationSessions', () => ({
  useNavigationSessions: vi.fn(),
}));

vi.mock('react-router-dom', () => ({
  useLocation: () => ({ pathname: '/' }),
  useNavigate: () => vi.fn(),
}));

vi.mock('../workspaces/WorkspaceSidebarSection', () => ({
  WorkspaceSidebarSection: () => <div data-testid="workspaces">Workspaces</div>,
}));

const WORKSPACES_HEIGHT_KEY = 'workspaces_sidebar_height';

type ElementRect = ReturnType<HTMLElement['getBoundingClientRect']>;

// `fireEvent` needs a constructed event and the lint config's global allowlist
// has no PointerEvent; the handlers only read `clientY`, which a MouseEvent
// carries, and the listeners key off the event name.
const pointerEvent = (name: string, clientY?: number) =>
  new window.MouseEvent(name, clientY === undefined ? undefined : { clientY });

// The panel is laid out by flexbox, which jsdom does not run, so the divider's
// clamp is exercised through stubbed geometry: the pane starts at y=100 and the
// panel ends at y=700, leaving 480px of travel above the chats minimum.
function stubGeometry(paneHeight: number) {
  const pane = screen.getByTestId('workspaces').parentElement as HTMLElement;
  const panel = pane.parentElement as HTMLElement;
  vi.spyOn(pane, 'getBoundingClientRect').mockReturnValue({
    top: 100,
    bottom: 100 + paneHeight,
    height: paneHeight,
  } as ElementRect);
  vi.spyOn(panel, 'getBoundingClientRect').mockReturnValue({
    top: 0,
    bottom: 700,
    height: 700,
  } as ElementRect);
  return pane;
}

describe('NavigationPanel workspaces/chats divider', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.mocked(useNavigationContext).mockReturnValue({
      isNavExpanded: true,
    } as ReturnType<typeof useNavigationContext>);
    vi.mocked(useNavigationSessions).mockReturnValue({
      recentSessions: [],
      activeSessionId: null,
      fetchSessions: vi.fn(),
      handleNavClick: vi.fn(),
      handleSessionClick: vi.fn(),
    } as unknown as ReturnType<typeof useNavigationSessions>);
  });

  const renderPanel = () =>
    render(
      <IntlTestWrapper>
        <Navigation />
      </IntlTestWrapper>
    );

  it('sizes the workspaces list with its content until the divider is dragged', () => {
    renderPanel();

    const pane = screen.getByTestId('workspaces').parentElement as HTMLElement;
    expect(pane.className).toContain('max-h-[45%]');
    expect(pane.style.height).toBe('');
  });

  it('drags the divider to a new split and remembers it', () => {
    renderPanel();
    const pane = stubGeometry(200);
    const divider = screen.getByRole('separator');

    fireEvent.pointerDown(divider, { clientY: 300 });
    fireEvent(window, pointerEvent('pointermove', 380));
    fireEvent(window, pointerEvent('pointerup'));

    expect(pane.style.height).toBe('280px');
    expect(pane.className).not.toContain('max-h-[45%]');
    expect(window.localStorage.getItem(WORKSPACES_HEIGHT_KEY)).toBe('280');
  });

  it('will not drag a section out of existence', () => {
    renderPanel();
    const pane = stubGeometry(200);
    const divider = screen.getByRole('separator');

    // Far past the top: the workspaces list keeps its minimum.
    fireEvent.pointerDown(divider, { clientY: 300 });
    fireEvent(window, pointerEvent('pointermove', -900));
    fireEvent(window, pointerEvent('pointerup'));
    expect(pane.style.height).toBe('72px');

    // Far past the bottom: the chats list keeps its minimum (700 - 100 - 120).
    fireEvent.pointerDown(divider, { clientY: 300 });
    fireEvent(window, pointerEvent('pointermove', 9000));
    fireEvent(window, pointerEvent('pointerup'));
    expect(pane.style.height).toBe('480px');
  });

  it('moves the divider with the arrow keys', () => {
    renderPanel();
    const pane = stubGeometry(200);
    const divider = screen.getByRole('separator');

    fireEvent.keyDown(divider, { key: 'ArrowDown' });
    expect(pane.style.height).toBe('224px');
    expect(window.localStorage.getItem(WORKSPACES_HEIGHT_KEY)).toBe('224');
  });

  it('restores content sizing on a double-click', () => {
    window.localStorage.setItem(WORKSPACES_HEIGHT_KEY, '300');
    renderPanel();

    const pane = screen.getByTestId('workspaces').parentElement as HTMLElement;
    expect(pane.style.height).toBe('300px');

    fireEvent.doubleClick(screen.getByRole('separator'));

    expect(pane.style.height).toBe('');
    expect(pane.className).toContain('max-h-[45%]');
    expect(window.localStorage.getItem(WORKSPACES_HEIGHT_KEY)).toBeNull();
  });
});
