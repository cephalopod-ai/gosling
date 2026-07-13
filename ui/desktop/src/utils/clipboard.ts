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
