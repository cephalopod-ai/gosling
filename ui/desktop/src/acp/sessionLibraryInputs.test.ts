import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getAcpClient } from './acpConnection';
import { addResearchInitialInputs, resolveSessionLibraryInputs } from './sessionLibraryInputs';

vi.mock('./acpConnection', () => ({ getAcpClient: vi.fn() }));

const addText = vi.fn();
const linkFile = vi.fn();
const resolve = vi.fn();

describe('session library research inputs', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getAcpClient).mockResolvedValue({
      gosling: {
        shellSessionLibraryAddText_unstable: addText,
        shellSessionLibraryLinkFile_unstable: linkFile,
        shellSessionLibraryResolve_unstable: resolve,
      },
    } as never);
    addText.mockResolvedValue({ item: { id: 'notes' } });
    linkFile
      .mockResolvedValueOnce({ item: { id: 'file-one' } })
      .mockResolvedValueOnce({ item: { id: 'file-two' } });
  });

  it('stores pasted content and every selected file in session scope', async () => {
    await expect(
      addResearchInitialInputs('research-session', {
        text: 'Compare the reports with https://example.com.',
        files: [
          { id: 'one', name: 'one.pdf', path: '/inputs/one.pdf', sizeBytes: 100 },
          { id: 'two', name: 'two.txt', path: '/inputs/two.txt', sizeBytes: 200 },
        ],
      })
    ).resolves.toEqual(['notes', 'file-one', 'file-two']);

    expect(addText).toHaveBeenCalledWith({
      sessionId: 'research-session',
      scope: 'session',
      name: 'Initial research notes',
      text: 'Compare the reports with https://example.com.',
    });
    expect(linkFile).toHaveBeenNthCalledWith(1, {
      sessionId: 'research-session',
      scope: 'session',
      path: '/inputs/one.pdf',
    });
    expect(linkFile).toHaveBeenNthCalledWith(2, {
      sessionId: 'research-session',
      scope: 'session',
      path: '/inputs/two.txt',
    });
  });

  it('resolves stored text and images for the first agent message', async () => {
    resolve.mockResolvedValue({
      items: [
        { id: 'notes', name: 'Notes', content: { type: 'text', text: 'Source notes' } },
        {
          id: 'figure',
          name: 'Figure',
          content: { type: 'image', data: 'aW1hZ2U=', mime_type: 'image/png' },
        },
      ],
    });

    await expect(
      resolveSessionLibraryInputs('research-session', ['notes', 'figure'])
    ).resolves.toEqual({
      assistantContext: 'Source notes',
      images: [{ data: 'aW1hZ2U=', mimeType: 'image/png' }],
    });
  });

  it('rejects more inputs than the session prompt boundary permits', async () => {
    await expect(
      addResearchInitialInputs('research-session', {
        text: 'One pasted input',
        files: Array.from({ length: 16 }, (_, index) => ({
          id: String(index),
          name: `${index}.txt`,
          path: `/inputs/${index}.txt`,
          sizeBytes: 1,
        })),
      })
    ).rejects.toThrow('up to 16 initial inputs');
    expect(getAcpClient).not.toHaveBeenCalled();
  });
});
