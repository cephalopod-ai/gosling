import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { saveArtifactWithDialog } from './artifactSave';

describe('saveArtifactWithDialog', () => {
  it('writes UTF-8 and base64 content only after the user selects a destination', async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), 'gosling-artifact-save-'));
    const textPath = path.join(directory, 'report.md');
    const imagePath = path.join(directory, 'pixel.bin');
    const showSaveDialog = vi
      .fn()
      .mockResolvedValueOnce({ canceled: false, filePath: textPath })
      .mockResolvedValueOnce({ canceled: false, filePath: imagePath });
    const dependencies = { resolveSource: vi.fn(), showSaveDialog };

    await saveArtifactWithDialog(
      {
        defaultPath: '/workspace/Documents/report.md',
        source: { type: 'content', content: '# Report', encoding: 'utf8' },
      },
      dependencies
    );
    await saveArtifactWithDialog(
      {
        defaultPath: '/workspace/Images/pixel.bin',
        source: { type: 'content', content: 'AAEC', encoding: 'base64' },
      },
      dependencies
    );

    expect(await readFile(textPath, 'utf8')).toBe('# Report');
    expect([...(await readFile(imagePath))]).toEqual([0, 1, 2]);
  });

  it('authorizes and copies file sources without moving the original', async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), 'gosling-artifact-copy-'));
    const sourcePath = path.join(directory, 'source.pdf');
    const targetPath = path.join(directory, 'copy.pdf');
    await writeFile(sourcePath, 'pdf');
    const resolveSource = vi.fn().mockResolvedValue(sourcePath);

    await saveArtifactWithDialog(
      {
        defaultPath: targetPath,
        source: { type: 'file', path: 'source.pdf', baseDirectory: directory },
      },
      {
        resolveSource,
        showSaveDialog: vi.fn().mockResolvedValue({ canceled: false, filePath: targetPath }),
      }
    );

    expect(resolveSource).toHaveBeenCalledWith('source.pdf', directory);
    expect(await readFile(sourcePath, 'utf8')).toBe('pdf');
    expect(await readFile(targetPath, 'utf8')).toBe('pdf');
  });

  it('does not resolve or write a source when the dialog is canceled', async () => {
    const resolveSource = vi.fn();
    const result = await saveArtifactWithDialog(
      {
        defaultPath: '/outputs/report.pdf',
        source: { type: 'file', path: '/workspace/report.pdf' },
      },
      {
        resolveSource,
        showSaveDialog: vi.fn().mockResolvedValue({ canceled: true }),
      }
    );
    expect(result).toEqual({ canceled: true });
    expect(resolveSource).not.toHaveBeenCalled();
  });

  it('rejects malformed base64 instead of presenting a partial artifact as saved', async () => {
    const directory = await mkdtemp(path.join(os.tmpdir(), 'gosling-artifact-invalid-'));
    const targetPath = path.join(directory, 'broken.png');
    await expect(
      saveArtifactWithDialog(
        {
          defaultPath: targetPath,
          source: { type: 'content', content: 'not!base64', encoding: 'base64' },
        },
        {
          resolveSource: vi.fn(),
          showSaveDialog: vi.fn().mockResolvedValue({ canceled: false, filePath: targetPath }),
        }
      )
    ).rejects.toThrow('invalid base64');
    await expect(readFile(targetPath)).rejects.toMatchObject({ code: 'ENOENT' });
  });

  it('rejects oversized in-memory artifacts before opening a destination dialog', async () => {
    const showSaveDialog = vi.fn();
    await expect(
      saveArtifactWithDialog(
        {
          defaultPath: '/outputs/oversized.bin',
          source: {
            type: 'content',
            content: 'oversized',
            encoding: 'utf8',
          },
        },
        { resolveSource: vi.fn(), showSaveDialog, maxContentLength: 4 }
      )
    ).rejects.toThrow('too large');
    expect(showSaveDialog).not.toHaveBeenCalled();
  });
});
