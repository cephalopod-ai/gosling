import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import { ToolCallStatusIndicator, type ToolCallStatus } from './ToolCallStatusIndicator';

describe('ToolCallStatusIndicator', () => {
  it.each([
    ['pending', 'clock-3'],
    ['loading', 'loader-circle'],
    ['success', 'check'],
    ['error', 'x'],
  ] satisfies Array<[ToolCallStatus, string]>)('renders a distinct non-color cue for %s', (status, icon) => {
    render(<ToolCallStatusIndicator status={status} />, { wrapper: IntlTestWrapper });

    const indicator = screen.getByLabelText(`Tool status: ${status}`);
    expect(indicator).toHaveAttribute('data-status', status);
    expect(indicator.querySelector(`.lucide-${icon}`)).toBeInTheDocument();
  });
});
