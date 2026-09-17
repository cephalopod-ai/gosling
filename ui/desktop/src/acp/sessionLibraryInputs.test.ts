import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getAcpClient } from './acpConnection';
import {
  addResearchInitialInputs,
  addSessionLibraryText,
  compileSessionLibraryInputs,
  linkSessionLibraryFile,
  listSessionLibraryInputs,
  resolveSessionLibraryInputs,
} from './sessionLibraryInputs';

vi.mock('./acpConnection', () => ({ getAcpClient: vi.fn() }));

const addText = vi.fn();
const linkFile = vi.fn();
const list = vi.fn();
const resolve = vi.fn();
const compile = vi.fn();

describe('session library research inputs', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    list.mockResolvedValue({ items: [] });
    resolve.mockReset();
    compile.mockReset().mockResolvedValue({
      filePath: '/chat/compiled-inputs.md',
      sourceCount: 19,
      sizeBytes: 900_000,
      promptText: 'Read all 19 sources at /chat/compiled-inputs.md in sections.',
    });
    vi.mocked(getAcpClient).mockResolvedValue({
      gosling: {
        shellSessionLibraryAddText_unstable: addText,
        shellSessionLibraryLinkFile_unstable: linkFile,
        shellSessionLibraryList_unstable: list,
        shellSessionLibraryResolve_unstable: resolve,
        shellSessionLibraryCompile_unstable: compile,
      },
    } as never);
    addText.mockResolvedValue({ item: { id: 'notes' } });
    linkFile
      .mockResolvedValueOnce({ item: { id: 'file-one' } })
      .mockResolvedValueOnce({ item: { id: 'file-two' } });
  });

  it('lists the safe stored metadata for the active session', async () => {
    const items = [
      {
        id: 'notes',
        name: 'Initial research notes',
        kind: 'text',
        scope: 'session',
        status: 'available',
        mimeType: 'text/plain',
        sizeBytes: 42,
      },
    ];
    list.mockResolvedValue({ items });

    await expect(listSessionLibraryInputs('research-session')).resolves.toEqual(items);
    expect(list).toHaveBeenCalledWith({ sessionId: 'research-session' });
  });

  it('stores each pasted input and every selected file separately in session scope', async () => {
    addText
      .mockResolvedValueOnce({ item: { id: 'input-one' } })
      .mockResolvedValueOnce({ item: { id: 'input-two' } });

    await expect(
      addResearchInitialInputs('research-session', {
        texts: ['Compare the reports with https://example.com.', 'Second pasted report.'],
        files: [
          { id: 'one', name: 'one.pdf', path: '/inputs/one.pdf', sizeBytes: 100 },
          { id: 'two', name: 'two.txt', path: '/inputs/two.txt', sizeBytes: 200 },
        ],
      })
    ).resolves.toEqual(['input-one', 'input-two', 'file-one', 'file-two']);

    expect(addText).toHaveBeenNthCalledWith(1, {
      sessionId: 'research-session',
      scope: 'session',
      name: 'Initial research input 1',
      text: 'Compare the reports with https://example.com.',
    });
    expect(addText).toHaveBeenNthCalledWith(2, {
      sessionId: 'research-session',
      scope: 'session',
      name: 'Initial research input 2',
      text: 'Second pasted report.',
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
        texts: ['One pasted input'],
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

  it('compiles a 19-source selection without submitting an invalid inline request', async () => {
    const ids = Array.from({ length: 19 }, (_, index) => `source-${index}`);
    await expect(resolveSessionLibraryInputs('chat', ids)).resolves.toEqual({
      assistantContext: 'Read all 19 sources at /chat/compiled-inputs.md in sections.',
      images: [],
    });
    expect(compile).toHaveBeenCalledWith({ sessionId: 'chat', itemIds: ids });
    expect(resolve).not.toHaveBeenCalled();
  });

  it('preflights the combined stored text size and compiles without truncating', async () => {
    const items = Array.from({ length: 15 }, (_, index) => ({
      id: `s${index}`,
      kind: 'text',
      sizeBytes: 50_000,
    }));
    list.mockResolvedValue({ items });
    await resolveSessionLibraryInputs(
      'chat',
      items.map((item) => item.id)
    );
    expect(compile).toHaveBeenCalledOnce();
    expect(resolve).not.toHaveBeenCalled();
  });

  it('falls back to compilation when resolved linked-file content exceeds the inline limit', async () => {
    resolve.mockRejectedValue({
      message: 'Invalid params',
      data: { code: 'SHELL_LIBRARY_SELECTION_TOO_LARGE' },
    });
    await resolveSessionLibraryInputs('chat', ['linked-document']);
    expect(compile).toHaveBeenCalledWith({ sessionId: 'chat', itemIds: ['linked-document'] });
  });

  it('keeps exactly 512 KiB inline and excludes unselected sources from the budget', async () => {
    list.mockResolvedValue({
      items: [
        { id: 'one', kind: 'text', sizeBytes: 256 * 1024 },
        { id: 'two', kind: 'text', sizeBytes: 256 * 1024 },
        { id: 'unselected', kind: 'text', sizeBytes: 256 * 1024 },
      ],
    });
    resolve.mockResolvedValue({ items: [] });
    await resolveSessionLibraryInputs('chat', ['one', 'two']);
    expect(resolve).toHaveBeenCalledWith({ sessionId: 'chat', itemIds: ['one', 'two'] });
    expect(compile).not.toHaveBeenCalled();
  });

  it('does not hide missing-source failures or compilation failures', async () => {
    const error = { message: 'Invalid params', data: { code: 'SHELL_LIBRARY_ITEM_UNAVAILABLE' } };
    resolve.mockRejectedValue(error);
    await expect(resolveSessionLibraryInputs('chat', ['missing'])).rejects.toEqual(error);
    expect(compile).not.toHaveBeenCalled();
    compile.mockRejectedValue(new Error('Compilation too large'));
    await expect(compileSessionLibraryInputs('chat', ['a'])).rejects.toThrow(
      'Compilation too large'
    );
  });

  it('rejects text and image byte limits before calling ACP', async () => {
    await expect(
      addResearchInitialInputs('research-session', {
        texts: ['😀'.repeat(70_000)],
        files: [],
      })
    ).rejects.toThrow('text exceeds the ACP input limits');

    await expect(
      addResearchInitialInputs('research-session', {
        texts: [],
        files: [
          {
            id: 'large-image',
            name: 'figure.png',
            path: '/inputs/figure.png',
            sizeBytes: 5 * 1024 * 1024 + 1,
          },
        ],
      })
    ).rejects.toThrow('files exceed the ACP input limits');
    expect(getAcpClient).not.toHaveBeenCalled();
  });

  it('adds standalone text in session scope preserving the original content', async () => {
    await addSessionLibraryText('chat', 'Source notes', '  notes\n');
    expect(addText).toHaveBeenCalledWith({
      sessionId: 'chat',
      scope: 'session',
      name: 'Source notes',
      text: '  notes\n',
    });
  });

  it('links only the file selected through the existing Electron bridge', async () => {
    Object.assign(window.electron, {
      getPathForFile: vi.fn().mockReturnValue('/selected/notes.txt'),
    });
    const file = new File(['notes'], 'notes.txt');
    await linkSessionLibraryFile('chat', file);
    expect(window.electron.getPathForFile).toHaveBeenCalledWith(file);
    expect(linkFile).toHaveBeenCalledWith({
      sessionId: 'chat',
      scope: 'session',
      path: '/selected/notes.txt',
    });
  });

  it('rejects empty or oversized individual inputs before calling ACP', async () => {
    await expect(addSessionLibraryText('chat', 'Notes', '  ')).rejects.toThrow(
      'must contain content'
    );
    await expect(addSessionLibraryText('chat', 'Notes', '😀'.repeat(70_000))).rejects.toThrow(
      '256 KB'
    );
    await expect(linkSessionLibraryFile('chat', new File([], 'empty.txt'))).rejects.toThrow(
      'non-empty'
    );
    const image = new File(['image'], 'image.png');
    Object.defineProperty(image, 'size', { value: 5 * 1024 * 1024 + 1 });
    await expect(linkSessionLibraryFile('chat', image)).rejects.toThrow('image up to 5 MB');
    expect(getAcpClient).not.toHaveBeenCalled();
  });
});
