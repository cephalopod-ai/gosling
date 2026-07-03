/**
 * @vitest-environment jsdom
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ArchiveFolderNotConfiguredError,
  archiveSessionToConfiguredFolder,
  getTrackedArchiveFile,
  removeTrackedArchiveFile,
} from './sessionArchive';

const acpArchiveSessionMock = vi.fn();
const acpExportSessionMock = vi.fn();

vi.mock('./acp/sessions', () => ({
  acpArchiveSession: (...args: unknown[]) => acpArchiveSessionMock(...args),
  acpExportSession: (...args: unknown[]) => acpExportSessionMock(...args),
}));

type ArchivedSessionFiles = Record<string, string>;

describe('sessionArchive', () => {
  let archiveFolder: string | null;
  let archivedSessionFiles: ArchivedSessionFiles;
  let setSettingMock: ReturnType<typeof vi.fn>;
  let deleteFileMock: ReturnType<typeof vi.fn>;
  let ensureDirectoryMock: ReturnType<typeof vi.fn>;
  let writeFileMock: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-07-02T14:15:16.789Z'));

    archiveFolder = '/tmp/gosling-archives';
    archivedSessionFiles = {};
    setSettingMock = vi.fn(async (key: string, value: unknown) => {
      if (key === 'archivedSessionFiles') {
        archivedSessionFiles = value as ArchivedSessionFiles;
      }
      if (key === 'archiveFolder') {
        archiveFolder = value as string | null;
      }
    });
    deleteFileMock = vi.fn().mockResolvedValue(true);
    ensureDirectoryMock = vi.fn().mockResolvedValue(true);
    writeFileMock = vi.fn().mockResolvedValue(true);
    acpArchiveSessionMock.mockReset().mockResolvedValue(undefined);
    acpExportSessionMock.mockReset().mockResolvedValue('{"id":"session-1"}');

    Object.assign(window, {
      electron: {
        getSetting: vi.fn(async (key: string) => {
          if (key === 'archiveFolder') {
            return archiveFolder;
          }
          if (key === 'archivedSessionFiles') {
            return archivedSessionFiles;
          }
          return null;
        }),
        setSetting: setSettingMock,
        ensureDirectory: ensureDirectoryMock,
        writeFile: writeFileMock,
        deleteFile: deleteFileMock,
      },
    });
  });

  it('exports, archives, and tracks the archive file path', async () => {
    const result = await archiveSessionToConfiguredFolder('session-1', 'Roadmap / Draft');

    expect(ensureDirectoryMock).toHaveBeenCalledWith('/tmp/gosling-archives');
    expect(writeFileMock).toHaveBeenCalledWith(
      '/tmp/gosling-archives/2026-07-02T14-15-16-789Z-Roadmap-Draft-session-1.json',
      '{"id":"session-1"}'
    );
    expect(acpArchiveSessionMock).toHaveBeenCalledWith('session-1');
    expect(result).toEqual({
      archivedAt: '2026-07-02T14:15:16.789Z',
      filePath: '/tmp/gosling-archives/2026-07-02T14-15-16-789Z-Roadmap-Draft-session-1.json',
    });
    expect(await getTrackedArchiveFile('session-1')).toBe(
      '/tmp/gosling-archives/2026-07-02T14-15-16-789Z-Roadmap-Draft-session-1.json'
    );
  });

  it('sanitizes invalid archive filename characters', async () => {
    await archiveSessionToConfiguredFolder('session-1', 'Bad\u0000<>:"/\\|?* name');

    expect(writeFileMock).toHaveBeenCalledWith(
      '/tmp/gosling-archives/2026-07-02T14-15-16-789Z-Bad-name-session-1.json',
      '{"id":"session-1"}'
    );
  });

  it('rejects archiving when no archive folder is configured', async () => {
    archiveFolder = null;

    await expect(archiveSessionToConfiguredFolder('session-1', 'Draft')).rejects.toBeInstanceOf(
      ArchiveFolderNotConfiguredError
    );
    expect(writeFileMock).not.toHaveBeenCalled();
    expect(acpArchiveSessionMock).not.toHaveBeenCalled();
  });

  it('always clears tracked archive metadata even when disk deletion fails', async () => {
    archivedSessionFiles = {
      'session-1': '/tmp/gosling-archives/archive-session-1.json',
    };
    deleteFileMock.mockResolvedValue(false);

    const result = await removeTrackedArchiveFile('session-1');

    expect(result).toEqual({
      filePath: '/tmp/gosling-archives/archive-session-1.json',
      hadTrackedFile: true,
      removed: false,
    });
    expect(deleteFileMock).toHaveBeenCalledWith('/tmp/gosling-archives/archive-session-1.json');
    expect(archivedSessionFiles).toEqual({});
  });
});
