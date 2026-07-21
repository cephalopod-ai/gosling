import { act, createRef } from 'react';
import { fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ScrollArea, type ScrollAreaHandle } from './scroll-area';

class TestResizeObserver {
  static instances: TestResizeObserver[] = [];

  readonly observe = vi.fn<(target: unknown) => void>();
  readonly unobserve = vi.fn<(target: unknown) => void>();
  readonly disconnect = vi.fn<() => void>();

  constructor(private readonly callback: ConstructorParameters<typeof window.ResizeObserver>[0]) {
    TestResizeObserver.instances.push(this);
  }

  trigger() {
    this.callback([], this);
  }
}

describe('ScrollArea auto-follow', () => {
  beforeEach(() => {
    TestResizeObserver.instances = [];
    vi.stubGlobal('ResizeObserver', TestResizeObserver);
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      callback(0);
      return 1;
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('stays pinned through viewport resizes until the user scrolls upward', () => {
    const ref = createRef<ScrollAreaHandle>();
    render(
      <ScrollArea ref={ref} autoScroll>
        <div>streaming response</div>
      </ScrollArea>
    );

    const viewport = ref.current?.viewportRef.current;
    expect(viewport).not.toBeNull();
    if (!viewport) return;

    let scrollHeight = 1_000;
    let clientHeight = 200;
    let scrollTop = 0;
    Object.defineProperties(viewport, {
      scrollHeight: { configurable: true, get: () => scrollHeight },
      clientHeight: { configurable: true, get: () => clientHeight },
      scrollTop: {
        configurable: true,
        get: () => scrollTop,
        set: (value: number) => {
          scrollTop = value;
        },
      },
    });
    const scrollTo = vi.fn(({ top }: { top?: number }) => {
      scrollTop = Math.min(Number(top), scrollHeight - clientHeight);
    });
    viewport.scrollTo = scrollTo as unknown as typeof viewport.scrollTo;

    const observer = TestResizeObserver.instances[TestResizeObserver.instances.length - 1];
    expect(observer).toBeDefined();
    expect(observer?.observe).toHaveBeenCalledWith(viewport);

    scrollTop = 300;
    fireEvent.scroll(viewport);
    act(() => observer?.trigger());
    expect(scrollTo).toHaveBeenLastCalledWith({ top: 1_000, behavior: 'auto' });

    fireEvent.scroll(viewport);
    scrollTop = 400;
    fireEvent.scroll(viewport);
    const callsBeforeResize = scrollTo.mock.calls.length;

    scrollHeight = 1_200;
    clientHeight = 150;
    act(() => TestResizeObserver.instances[TestResizeObserver.instances.length - 1]?.trigger());
    expect(scrollTo).toHaveBeenCalledTimes(callsBeforeResize);

    act(() => ref.current?.scrollToBottom('auto'));
    const callsAfterRepinning = scrollTo.mock.calls.length;
    scrollHeight = 1_300;
    act(() => TestResizeObserver.instances[TestResizeObserver.instances.length - 1]?.trigger());
    expect(scrollTo).toHaveBeenCalledTimes(callsAfterRepinning + 1);
  });
});
