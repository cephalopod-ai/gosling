import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import type { Message } from '../../types/message';
import ThreadNavigator, { THREAD_TURN_ATTRIBUTE } from './ThreadNavigator';

type ElementRect = ReturnType<HTMLElement['getBoundingClientRect']>;

class ResizeObserverMock {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}

function message(role: Message['role'], text: string, id: string): Message {
  return {
    id,
    role,
    content: [{ type: 'text', text }],
    created: 1,
    metadata: { agentVisible: true, userVisible: true },
  };
}

function renderNavigator(options?: { historyHasMore?: boolean; historyLoading?: boolean }) {
  const viewport = document.createElement('div');
  Object.defineProperty(viewport, 'clientHeight', { configurable: true, value: 500 });
  Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 1_500 });
  Object.defineProperty(viewport, 'scrollTop', { configurable: true, writable: true, value: 0 });
  viewport.getBoundingClientRect = vi.fn(() => ({ top: 0 }) as ElementRect);

  const firstTurn = document.createElement('div');
  firstTurn.setAttribute(THREAD_TURN_ATTRIBUTE, '0');
  firstTurn.getBoundingClientRect = vi.fn(() => ({ top: 40 }) as ElementRect);
  firstTurn.scrollIntoView = vi.fn();

  const secondTurn = document.createElement('div');
  secondTurn.setAttribute(THREAD_TURN_ATTRIBUTE, '2');
  secondTurn.getBoundingClientRect = vi.fn(() => ({ top: 400 }) as ElementRect);
  secondTurn.scrollIntoView = vi.fn();

  viewport.append(firstTurn, secondTurn);
  document.body.append(viewport);

  const onJumpToStart = vi.fn();
  const onJumpToLatest = vi.fn();
  render(
    <ThreadNavigator
      messages={[
        message('user', 'First prompt', 'user-one'),
        message('assistant', 'First answer', 'assistant-one'),
        message('user', 'Second prompt', 'user-two'),
        message('assistant', 'Second answer', 'assistant-two'),
      ]}
      viewport={viewport}
      historyHasMore={options?.historyHasMore}
      historyLoading={options?.historyLoading}
      onJumpToStart={onJumpToStart}
      onJumpToLatest={onJumpToLatest}
    />,
    { wrapper: IntlTestWrapper }
  );

  return { firstTurn, onJumpToLatest, onJumpToStart, secondTurn, viewport };
}

describe('ThreadNavigator', () => {
  beforeEach(() => {
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('renders user turns and jumps to each location', async () => {
    const { secondTurn } = renderNavigator();

    expect(screen.getByRole('navigation', { name: 'Thread navigation' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Turn 1 of 2: First prompt/ })).toHaveAttribute(
      'aria-current',
      'location'
    );

    await userEvent.click(screen.getByRole('button', { name: /Turn 2 of 2: Second prompt/ }));

    expect(secondTurn.scrollIntoView).toHaveBeenCalledWith({
      behavior: 'smooth',
      block: 'center',
    });
    expect(screen.getByRole('button', { name: /Turn 2 of 2: Second prompt/ })).toHaveAttribute(
      'aria-current',
      'location'
    );
  });

  it('uses roving focus and arrow keys across turn locations', async () => {
    renderNavigator();

    const firstTurn = screen.getByRole('button', { name: /Turn 1 of 2: First prompt/ });
    const secondTurn = screen.getByRole('button', { name: /Turn 2 of 2: Second prompt/ });
    expect(firstTurn).toHaveAttribute('tabindex', '0');
    expect(secondTurn).toHaveAttribute('tabindex', '-1');

    firstTurn.focus();
    await userEvent.keyboard('{ArrowDown}');

    expect(secondTurn).toHaveFocus();
    expect(secondTurn).toHaveAttribute('aria-current', 'location');
    expect(secondTurn).toHaveAttribute('tabindex', '0');
  });

  it('tracks the active turn as the viewport scrolls', async () => {
    const { secondTurn, viewport } = renderNavigator();
    secondTurn.getBoundingClientRect = vi.fn(() => ({ top: 80 }) as ElementRect);

    act(() => fireEvent.scroll(viewport));

    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: /Turn 2 of 2: Second prompt/ })).toHaveAttribute(
        'aria-current',
        'location'
      );
    });
  });

  it('selects the final turn at the bottom of the thread', async () => {
    const { viewport } = renderNavigator();
    viewport.scrollTop = 1_000;

    act(() => fireEvent.scroll(viewport));

    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: /Turn 2 of 2: Second prompt/ })).toHaveAttribute(
        'aria-current',
        'location'
      );
    });
  });

  it('does not select an unmounted final turn at the bottom of a progressive batch', async () => {
    const viewport = document.createElement('div');
    Object.defineProperty(viewport, 'clientHeight', { configurable: true, value: 500 });
    Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 500 });
    Object.defineProperty(viewport, 'scrollTop', { configurable: true, writable: true, value: 0 });
    viewport.getBoundingClientRect = vi.fn(() => ({ top: 0 }) as ElementRect);
    const firstTurn = document.createElement('div');
    firstTurn.setAttribute(THREAD_TURN_ATTRIBUTE, '0');
    firstTurn.getBoundingClientRect = vi.fn(() => ({ top: 40 }) as ElementRect);
    viewport.append(firstTurn);
    document.body.append(viewport);

    render(
      <ThreadNavigator
        messages={[
          message('user', 'First prompt', 'user-one'),
          message('assistant', 'First answer', 'assistant-one'),
          message('user', 'Second prompt', 'user-two'),
        ]}
        viewport={viewport}
        onJumpToStart={vi.fn()}
        onJumpToLatest={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: /Turn 1 of 2: First prompt/ })).toHaveAttribute(
        'aria-current',
        'location'
      );
    });
  });

  it('exposes start and latest controls and disables start while history loads', async () => {
    const { onJumpToLatest, onJumpToStart } = renderNavigator({
      historyHasMore: true,
      historyLoading: true,
    });

    const start = screen.getByRole('button', { name: 'Jump to start' });
    expect(start).toBeDisabled();
    await userEvent.click(start);
    expect(onJumpToStart).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole('button', { name: 'Jump to latest' }));
    expect(onJumpToLatest).toHaveBeenCalledOnce();
  });

  it('finishes progressive rendering before jumping to an unmounted turn', async () => {
    const onPrepareNavigation = vi.fn();

    const viewport = document.createElement('div');
    Object.defineProperty(viewport, 'clientHeight', { configurable: true, value: 500 });
    Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 1_500 });
    viewport.getBoundingClientRect = vi.fn(() => ({ top: 0 }) as ElementRect);
    const firstTurn = document.createElement('div');
    firstTurn.setAttribute(THREAD_TURN_ATTRIBUTE, '0');
    firstTurn.getBoundingClientRect = vi.fn(() => ({ top: 40 }) as ElementRect);
    viewport.append(firstTurn);
    document.body.append(viewport);

    render(
      <ThreadNavigator
        messages={[
          message('user', 'First prompt', 'user-one'),
          message('assistant', 'First answer', 'assistant-one'),
          message('user', 'Second prompt', 'user-two'),
        ]}
        viewport={viewport}
        onPrepareNavigation={onPrepareNavigation}
        onJumpToStart={vi.fn()}
        onJumpToLatest={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    await userEvent.click(screen.getByRole('button', { name: /Turn 2 of 2: Second prompt/ }));
    expect(onPrepareNavigation).toHaveBeenCalledWith(2);
  });

  it('keeps start and latest reachable for one long turn', async () => {
    const viewport = document.createElement('div');
    Object.defineProperty(viewport, 'clientHeight', { configurable: true, value: 500 });
    Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 1_500 });

    render(
      <ThreadNavigator
        messages={[message('user', 'Only prompt', 'user-one')]}
        viewport={viewport}
        onJumpToStart={vi.fn()}
        onJumpToLatest={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    await vi.waitFor(() => {
      expect(screen.getByRole('navigation', { name: 'Thread navigation' })).toBeInTheDocument();
    });
    expect(screen.getByRole('button', { name: 'Jump to start' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Jump to latest' })).toBeInTheDocument();
  });

  it('stays hidden when a single turn already fits', () => {
    const viewport = document.createElement('div');
    Object.defineProperty(viewport, 'clientHeight', { configurable: true, value: 500 });
    Object.defineProperty(viewport, 'scrollHeight', { configurable: true, value: 500 });

    render(
      <ThreadNavigator
        messages={[message('user', 'Only prompt', 'user-one')]}
        viewport={viewport}
        onJumpToStart={vi.fn()}
        onJumpToLatest={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    expect(screen.queryByRole('navigation', { name: 'Thread navigation' })).not.toBeInTheDocument();
  });

  it('honors reduced motion when jumping to a turn', async () => {
    vi.stubGlobal(
      'matchMedia',
      vi.fn(() => ({ matches: true }))
    );
    const { secondTurn } = renderNavigator();

    await userEvent.click(screen.getByRole('button', { name: /Turn 2 of 2: Second prompt/ }));

    expect(secondTurn.scrollIntoView).toHaveBeenCalledWith({
      behavior: 'auto',
      block: 'center',
    });
  });
});
