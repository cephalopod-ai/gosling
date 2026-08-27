import { fireEvent, render, screen } from '@testing-library/react';
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

  it('enforces ACP UTF-8 byte limits for pasted text', async () => {
    const user = userEvent.setup();
    render(<ResearchInitialInputsDialog value={{ texts: [], files: [] }} onApply={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: /Initial Inputs/ }));
    fireEvent.change(screen.getByLabelText('Paste content'), {
      target: { value: '😀'.repeat(70_000) },
    });

    expect(screen.getByRole('alert')).toHaveTextContent(
      'Each pasted input must be no larger than 256 KB.'
    );
    expect(screen.getByRole('button', { name: 'Done' })).toBeDisabled();
  });

  it('enforces the ACP aggregate text limit', async () => {
    const user = userEvent.setup();
    render(
      <ResearchInitialInputsDialog
        value={{ texts: ['a'.repeat(250 * 1024), 'b'.repeat(250 * 1024)], files: [] }}
        onApply={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    await user.click(screen.getByRole('button', { name: /Initial Inputs/ }));
    fireEvent.change(screen.getByLabelText('Paste content'), {
      target: { value: 'c'.repeat(20 * 1024) },
    });

    expect(screen.getByRole('alert')).toHaveTextContent(
      'Pasted inputs exceed the 512 KB total limit.'
    );
  });

  it('shows and enforces the smaller image file limit', async () => {
    const user = userEvent.setup();
    render(<ResearchInitialInputsDialog value={{ texts: [], files: [] }} onApply={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    await user.click(screen.getByRole('button', { name: /Initial Inputs/ }));
    const image = new File([new Uint8Array(5 * 1024 * 1024 + 1)], 'figure.png', {
      type: 'image/png',
    });
    fireEvent.change(screen.getByLabelText('Choose initial research files'), {
      target: { files: [image] },
    });

    expect(screen.getByRole('alert')).toHaveTextContent(
      'figure.png is larger than the 5 MB image limit.'
    );
    expect(screen.getByText(/images up to 5 MB each and 10 MB total/i)).toBeInTheDocument();
  });
});
