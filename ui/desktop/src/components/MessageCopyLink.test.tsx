import React, { useRef } from 'react';
import { fireEvent, render, screen, waitFor, type RenderOptions } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach } from 'vitest';
import MessageCopyLink from './MessageCopyLink';
import { IntlTestWrapper } from '../i18n/test-utils';

const renderWithIntl = (ui: React.ReactElement, options?: RenderOptions) =>
  render(ui, { wrapper: IntlTestWrapper, ...options });

function MessageCopyLinkHarness({
  text,
  includeButton = true,
}: {
  text: string;
  includeButton?: boolean;
}) {
  const contentRef = useRef<HTMLDivElement | null>(null);

  return (
    <>
      <div ref={contentRef}>
        <p>{text}</p>
        {includeButton ? <button type="button">remove me</button> : null}
      </div>
      <MessageCopyLink text={text} contentRef={contentRef} />
    </>
  );
}

describe('MessageCopyLink', () => {
  beforeEach(() => {
    vi.mocked(window.electron.writeClipboardText).mockClear();
    vi.mocked(window.electron.writeClipboardHtml).mockClear();
  });

  it('uses the Electron HTML clipboard bridge and strips embedded buttons', async () => {
    renderWithIntl(<MessageCopyLinkHarness text="copied text" />);

    fireEvent.click(screen.getByRole('button', { name: 'Copy' }));

    await waitFor(() => {
      expect(window.electron.writeClipboardHtml).toHaveBeenCalledWith(
        '<p>copied text</p>',
        'copied text'
      );
    });
    expect(window.electron.writeClipboardText).not.toHaveBeenCalled();
  });

  it('falls back to plain text when HTML copy is unavailable', async () => {
    vi.mocked(window.electron.writeClipboardHtml).mockRejectedValueOnce(new Error('copy failed'));

    renderWithIntl(<MessageCopyLinkHarness text="plain text only" includeButton={false} />);

    fireEvent.click(screen.getByRole('button', { name: 'Copy' }));

    await waitFor(() => {
      expect(window.electron.writeClipboardText).toHaveBeenCalledWith('plain text only');
    });
  });
});
