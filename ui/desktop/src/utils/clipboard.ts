export async function writeTextToClipboard(text: string): Promise<void> {
  if (window.electron?.writeClipboardText) {
    await window.electron.writeClipboardText(text);
    return;
  }

  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  throw new Error('Clipboard text API unavailable');
}

export async function writeRichTextToClipboard(html: string, text: string): Promise<void> {
  if (window.electron?.writeClipboardHtml) {
    try {
      await window.electron.writeClipboardHtml(html, text);
      return;
    } catch {
      await writeTextToClipboard(text);
      return;
    }
  }

  if (
    typeof ClipboardItem !== 'undefined' &&
    navigator.clipboard?.write &&
    typeof Blob !== 'undefined'
  ) {
    const clipboardData = new ClipboardItem({
      'text/plain': new Blob([text], { type: 'text/plain' }),
      'text/html': new Blob([html], { type: 'text/html' }),
    });
    await navigator.clipboard.write([clipboardData]);
    return;
  }

  await writeTextToClipboard(text);
}
