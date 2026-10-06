import { useEffect, useRef } from 'react';

interface ResizeHandleProps {
  label: string;
  orientation: 'horizontal' | 'vertical';
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
  reverse?: boolean;
  className: string;
}

export function ResizeHandle({
  label,
  orientation,
  value,
  min,
  max,
  onChange,
  reverse = false,
  className,
}: ResizeHandleProps) {
  const cleanup = useRef<(() => void) | null>(null);
  useEffect(() => () => cleanup.current?.(), []);
  const update = (next: number) => onChange(Math.min(max, Math.max(min, next)));
  const vertical = orientation === 'vertical';

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={orientation}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      tabIndex={0}
      className={`no-drag touch-none ${className}`}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        cleanup.current?.();
        const start = vertical ? event.clientX : event.clientY;
        const move = (next: globalThis.PointerEvent) => {
          if (next.pointerId !== event.pointerId) return;
          const delta = (vertical ? next.clientX : next.clientY) - start;
          update(value + delta * (reverse ? -1 : 1));
        };
        const stop = (next: globalThis.PointerEvent) => {
          if (next.pointerId === event.pointerId) cleanup.current?.();
        };
        cleanup.current = () => {
          window.removeEventListener('pointermove', move);
          window.removeEventListener('pointerup', stop);
          window.removeEventListener('pointercancel', stop);
          cleanup.current = null;
        };
        window.addEventListener('pointermove', move);
        window.addEventListener('pointerup', stop);
        window.addEventListener('pointercancel', stop);
      }}
      onKeyDown={(event) => {
        const decrease = vertical ? 'ArrowLeft' : 'ArrowUp';
        const increase = vertical ? 'ArrowRight' : 'ArrowDown';
        if (event.key !== decrease && event.key !== increase) return;
        event.preventDefault();
        update(value + (event.key === increase ? 16 : -16) * (reverse ? -1 : 1));
      }}
    />
  );
}
