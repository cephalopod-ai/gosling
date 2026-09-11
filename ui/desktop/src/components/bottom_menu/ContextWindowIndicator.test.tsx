import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ContextWindowIndicator } from './ContextWindowIndicator';

describe('ContextWindowIndicator', () => {
  it('labels provider-reported usage as the last request', () => {
    render(<ContextWindowIndicator totalTokens={296_000} tokenLimit={258_400} alerts={[]} />);

    const usage = screen.getByLabelText('Last model request: 296k of 258k effective context limit');
    expect(usage).toHaveTextContent('296k req / 258k');
    expect(usage).toHaveClass('text-red-500');
  });

  it('shows the active estimate separately from the last model request', () => {
    render(
      <ContextWindowIndicator
        totalTokens={758_000}
        tokenLimit={998_000}
        alerts={[]}
        estimated
        lastRequestTokens={29_000}
      />
    );

    const usage = screen.getByLabelText(
      'Active context estimate: 758k of 998k effective context limit. Last model request: 29k'
    );
    expect(usage).toHaveTextContent('758k est / 998k');
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
    expect(usage).toHaveTextContent('1M req / 1M');
    expect(usage).toHaveClass('text-text-primary/70');
    expect(usage).not.toHaveClass('text-red-500');
  });
});
