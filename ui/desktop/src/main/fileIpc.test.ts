import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { desktopCommandChannels } from '../ipc/channels';
import { FILE_IPC_CHANNELS, registerFileIpcHandlers } from './fileIpc';

const bridge = vi.hoisted(() => ({ exposeInMainWorld: vi.fn(), invoke: vi.fn() }));

vi.mock('electron', () => ({
  contextBridge: { exposeInMainWorld: bridge.exposeInMainWorld },
  ipcRenderer: { invoke: bridge.invoke },
  webUtils: {},
  clipboard: { write: vi.fn(), writeText: vi.fn() },
  dialog: { showMessageBox: vi.fn(), showOpenDialog: vi.fn(), showSaveDialog: vi.fn() },
  shell: { openPath: vi.fn(), showItemInFolder: vi.fn() },
}));

const temporaryDirectories: string[] = [];

async function temporaryDirectory(): Promise<string> {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-file-ipc-'));
  temporaryDirectories.push(directory);
  return directory;
}

afterEach(async () => {
  await Promise.all(
    temporaryDirectories
      .splice(0)
      .map((directory) => fs.rm(directory, { recursive: true, force: true }))
  );
});

describe('file IPC registration', () => {
  it('registers every original file and artifact channel once', () => {
    const handle = vi.fn();
    registerFileIpcHandlers(
      { handle },
      {
        assertRendererFileAccess: vi.fn(),
        assertRendererArtifactFileAccess: vi.fn(),
        resolveRendererPath: vi.fn(),
        grantRendererDirectory: vi.fn(),
        grantRendererArtifactFile: vi.fn(),
        updateArtifactRoutingConfig: vi.fn(),
        getAllowList: vi.fn(),
      }
    );

    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(FILE_IPC_CHANNELS);
  });

  it('routes the artifact preload operations to registered native handlers', async () => {
    const handle = vi.fn();
    registerFileIpcHandlers(
      { handle },
      {
        assertRendererFileAccess: vi.fn(),
        assertRendererArtifactFileAccess: vi.fn(),
        resolveRendererPath: vi.fn(),
        grantRendererDirectory: vi.fn(),
        grantRendererArtifactFile: vi.fn(),
        updateArtifactRoutingConfig: vi.fn(),
        getAllowList: vi.fn(),
      }
    );
    const handlers = new Map(handle.mock.calls.map(([channel, handler]) => [channel, handler]));
    await import('../preload');
    const api = bridge.exposeInMainWorld.mock.calls.find(
      ([name]) => name === 'electron'
    )?.[1] as typeof window.electron;
    bridge.invoke.mockImplementation(async (channel: string) => {
      expect(handlers.has(channel)).toBe(true);
    });

    await api.copyArtifactContents('/output/report.md', '/output');
    await api.classifyArtifactRepositories(['/output/report.md']);
    await api.getArtifactFileTimestamps(['/output/report.md']);
    await api.trashArtifactFiles(['/output/report.md']);

    expect(bridge.invoke.mock.calls).toEqual([
      ['copy-artifact-contents', '/output/report.md', '/output'],
      ['classify-artifact-repositories', ['/output/report.md']],
      ['get-artifact-file-timestamps', ['/output/report.md']],
      ['trash-artifact-files', ['/output/report.md']],
    ]);
  });

  it('reports a directory symlink as a directory, not a file', async () => {
    const root = await temporaryDirectory();
    await fs.mkdir(path.join(root, 'real_dir'));
    await fs.writeFile(path.join(root, 'plain_file.txt'), 'hello');
    await fs.symlink(
      path.join(root, 'real_dir'),
      path.join(root, 'link_to_dir'),
      'dir'
    );
    await fs.symlink(path.join(root, 'missing_target'), path.join(root, 'broken_link'), 'dir');

    const handle = vi.fn();
    registerFileIpcHandlers(
      { handle },
      {
        assertRendererFileAccess: vi.fn(async (_senderId: number, requestedPath: string) => requestedPath),
        assertRendererArtifactFileAccess: vi.fn(),
        resolveRendererPath: vi.fn(),
        grantRendererDirectory: vi.fn(),
        grantRendererArtifactFile: vi.fn(),
        updateArtifactRoutingConfig: vi.fn(),
        getAllowList: vi.fn(),
      }
    );
    const listFilesHandler = handle.mock.calls.find(
      ([channel]) => channel === desktopCommandChannels.listFiles
    )?.[1];
    expect(listFilesHandler).toBeDefined();

    const entries = (await listFilesHandler!({ sender: { id: 1 } }, root)) as Array<{
      name: string;
      isDirectory: boolean;
    }>;
    const byName = new Map(entries.map((entry) => [entry.name, entry.isDirectory]));

    expect(byName.get('real_dir')).toBe(true);
    expect(byName.get('plain_file.txt')).toBe(false);
    // A Dirent reports the symlink's own type, not its target's — the
    // handler must resolve this itself rather than reporting every
    // symlinked directory as a file.
    expect(byName.get('link_to_dir')).toBe(true);
    // A dangling symlink can't be browsed into either way.
    expect(byName.get('broken_link')).toBe(false);
  });
});
