import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ContextWindowIndicator } from './ContextWindowIndicator';

describe('ContextWindowIndicator', () => {
  it('labels usage as the last request against the effective route limit', () => {
    render(<ContextWindowIndicator totalTokens={296_000} tokenLimit={258_400} alerts={[]} />);

    const usage = screen.getByLabelText('Last model request: 296k of 258k effective context limit');
    expect(usage).toHaveTextContent('296k / 258k');
    expect(usage).toHaveClass('text-red-500');
  });

  it('does not escalate to orange/red for a provider that manages its own context', () => {
    render(
      <ContextWindowIndicator
        totalTokens={1_000_000}
        tokenLimit={1_000_000}
        alerts={[]}
        managesOwnContext
      />
    );

    const usage = screen.getByLabelText(
      'Context managed by the connected CLI tool. Last request: 1M of 1M effective context limit'
    );
    expect(usage).toHaveTextContent('1M / 1M');
    expect(usage).toHaveClass('text-text-primary/70');
    expect(usage).not.toHaveClass('text-red-500');
  });
});
