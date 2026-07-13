import { describe, it, expect, vi, beforeEach } from 'vitest';
import { writeTextToClipboard, writeRichTextToClipboard } from './clipboard';

describe('clipboard utils', () => {
  beforeEach(() => {
    vi.mocked(window.electron.writeClipboardText).mockClear();
    vi.mocked(window.electron.writeClipboardHtml).mockClear();
    vi.mocked(navigator.clipboard.writeText).mockClear();
  });

  it('writeTextToClipboard prefers the Electron bridge over navigator.clipboard', async () => {
    await writeTextToClipboard('hello');

    expect(window.electron.writeClipboardText).toHaveBeenCalledWith('hello');
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
  });

  it('writeRichTextToClipboard prefers the Electron HTML bridge', async () => {
    await writeRichTextToClipboard('<p>hi</p>', 'hi');

    expect(window.electron.writeClipboardHtml).toHaveBeenCalledWith('<p>hi</p>', 'hi');
    expect(window.electron.writeClipboardText).not.toHaveBeenCalled();
  });

  it('writeRichTextToClipboard falls back to plain text when the HTML bridge fails', async () => {
    vi.mocked(window.electron.writeClipboardHtml).mockRejectedValueOnce(new Error('nope'));

    await writeRichTextToClipboard('<p>hi</p>', 'hi');

    expect(window.electron.writeClipboardText).toHaveBeenCalledWith('hi');
  });
});
