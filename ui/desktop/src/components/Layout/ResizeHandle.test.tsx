import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ResizeHandle } from './ResizeHandle';

describe('ResizeHandle', () => {
  it('clamps dragging, supports keyboard input and cleans up on cancellation', () => {
    const onChange = vi.fn();
    render(
      <ResizeHandle
        label="Resize"
        orientation="vertical"
        value={240}
        min={200}
        max={480}
        onChange={onChange}
        className=""
      />
    );
    const divider = screen.getByRole('separator');
    fireEvent.pointerDown(divider, { button: 0, pointerId: 1, clientX: 240 });
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 900 });
    expect(onChange).toHaveBeenLastCalledWith(480);
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 0 });
    expect(onChange).toHaveBeenLastCalledWith(200);
    fireEvent.pointerCancel(window, { pointerId: 1 });
    onChange.mockClear();
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 300 });
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.keyDown(divider, { key: 'ArrowRight' });
    expect(onChange).toHaveBeenLastCalledWith(256);
  });

  it('reverses right panel dragging and removes listeners on unmount', () => {
    const onChange = vi.fn();
    const { unmount } = render(
      <ResizeHandle
        label="Resize"
        orientation="vertical"
        value={400}
        min={320}
        max={720}
        reverse
        onChange={onChange}
        className=""
      />
    );
    fireEvent.pointerDown(screen.getByRole('separator'), { button: 0, pointerId: 1, clientX: 400 });
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 360 });
    expect(onChange).toHaveBeenLastCalledWith(440);
    unmount();
    onChange.mockClear();
    fireEvent.pointerMove(window, { pointerId: 1, clientX: 300 });
    expect(onChange).not.toHaveBeenCalled();
  });

  it('uses the Y axis for vertical section resizing', () => {
    const onChange = vi.fn();
    render(
      <ResizeHandle
        label="Resize"
        orientation="horizontal"
        value={240}
        min={80}
        max={600}
        onChange={onChange}
        className=""
      />
    );
    fireEvent.pointerDown(screen.getByRole('separator'), { button: 0, pointerId: 1, clientY: 240 });
    fireEvent.pointerMove(window, { pointerId: 1, clientY: 300 });
    expect(onChange).toHaveBeenLastCalledWith(300);
    fireEvent.pointerUp(window, { pointerId: 1 });
    fireEvent.keyDown(screen.getByRole('separator'), { key: 'ArrowUp' });
    expect(onChange).toHaveBeenLastCalledWith(224);
  });
});
