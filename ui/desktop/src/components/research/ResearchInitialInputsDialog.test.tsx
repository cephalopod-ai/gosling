import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { ResearchInitialInputsDialog } from './ResearchInitialInputsDialog';

describe('ResearchInitialInputsDialog', () => {
  it('keeps multiline and unbroken pasted text inside the dialog', async () => {
    const user = userEvent.setup();
    const longInput = `# Research report\n\n- First finding\n\nhttps://${'evidence'.repeat(80)}`;

    render(
      <ResearchInitialInputsDialog value={{ texts: [longInput], files: [] }} onApply={vi.fn()} />,
      { wrapper: IntlTestWrapper }
    );

    await user.click(screen.getByRole('button', { name: /Initial Inputs/ }));

    expect(screen.getByRole('dialog')).toHaveClass('min-w-0', 'overflow-x-hidden');
    expect(screen.getByLabelText('Paste content')).toHaveAttribute('wrap', 'soft');
    expect(screen.getByLabelText('Paste content')).toHaveClass(
      'min-w-0',
      'whitespace-pre-wrap',
      'break-words',
      '[overflow-wrap:anywhere]'
    );

    const preview = screen.getByText(
      (_content, element) => element?.tagName === 'P' && element.textContent === longInput
    );
    expect(preview).toHaveClass('whitespace-pre-wrap', 'break-words', '[overflow-wrap:anywhere]');
    expect(screen.getByRole('list', { name: 'Pasted inputs' })).toHaveClass(
      'min-w-0',
      'max-w-full',
      'overflow-x-hidden'
    );
    expect(preview.closest('li')).toHaveClass('w-full', 'min-w-0');
  });
});
