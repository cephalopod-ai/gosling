import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { ResponseStylesSection } from './ResponseStylesSection';

describe('ResponseStylesSection', () => {
  it('names each response style radio and selects it by name', async () => {
    const user = userEvent.setup();
    render(<ResponseStylesSection />, { wrapper: IntlTestWrapper });

    await waitFor(() => expect(screen.getByRole('radio', { name: 'Concise' })).toBeChecked());
    await user.click(screen.getByRole('radio', { name: 'Detailed' }));

    expect(screen.getByRole('radio', { name: 'Detailed' })).toBeChecked();
    expect(window.electron.setSetting).toHaveBeenCalledWith('responseStyle', 'detailed');
  });
});
