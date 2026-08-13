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
});
