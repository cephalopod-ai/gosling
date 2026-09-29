import { act, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { NavigationProvider, useNavigationContext } from './NavigationContext';

const STORAGE_KEY = 'navigation_expanded';

function setWindowWidth(width: number) {
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: width });
}

function resizeTo(width: number) {
  act(() => {
    setWindowWidth(width);
    window.dispatchEvent(new Event('resize'));
  });
}

let context: ReturnType<typeof useNavigationContext>;

function Probe() {
  context = useNavigationContext();
  return <span data-testid="nav">{context.isNavExpanded ? 'expanded' : 'collapsed'}</span>;
}

const renderProvider = () =>
  render(
    <NavigationProvider>
      <Probe />
    </NavigationProvider>
  );

const navState = () => screen.getByTestId('nav').textContent;

describe('NavigationProvider narrow-window collapse', () => {
  beforeEach(() => {
    window.localStorage.clear();
    setWindowWidth(940);
    window.electron = {
      ...window.electron,
      on: vi.fn(),
      off: vi.fn(),
    } as unknown as typeof window.electron;
  });

  it('does not save an automatic collapse as the user preference', () => {
    window.localStorage.setItem(STORAGE_KEY, 'true');
    renderProvider();

    resizeTo(600);

    expect(navState()).toBe('collapsed');
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe('true');
  });

  it('restores the navigation when the window widens again after an automatic collapse', () => {
    renderProvider();

    resizeTo(600);
    resizeTo(940);

    expect(navState()).toBe('expanded');
    expect(window.localStorage.getItem(STORAGE_KEY)).toBeNull();
  });

  it('does not save the collapse applied to a window that opens narrow', () => {
    window.localStorage.setItem(STORAGE_KEY, 'true');
    setWindowWidth(600);
    renderProvider();

    expect(navState()).toBe('collapsed');
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe('true');

    resizeTo(940);
    expect(navState()).toBe('expanded');
  });

  it('keeps an explicit collapse when the window widens', () => {
    renderProvider();

    act(() => context.setIsNavExpanded(false));
    resizeTo(600);
    resizeTo(940);

    expect(navState()).toBe('collapsed');
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe('false');
  });

  it('treats a manual choice made while narrow as the new preference', () => {
    renderProvider();

    resizeTo(600);
    act(() => context.setIsNavExpanded(true));
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe('true');

    act(() => context.setIsNavExpanded(false));
    resizeTo(940);

    expect(navState()).toBe('collapsed');
    expect(window.localStorage.getItem(STORAGE_KEY)).toBe('false');
  });
});
