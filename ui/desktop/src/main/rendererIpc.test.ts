import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { dialog } from 'electron';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { RendererDirectoryGrantRegistry } from '../utils/rendererDirectoryGrants';
import {
  loopbackHttpBaseFromAcpUrl,
  registerRendererIpcHandlers,
  RENDERER_IPC_HANDLE_CHANNELS,
  RENDERER_IPC_ON_CHANNELS,
} from './rendererIpc';

vi.mock('electron', () => ({
  app: { getPath: vi.fn(() => '/tmp') },
  BrowserWindow: {},
  dialog: { showOpenDialog: vi.fn() },
}));

const temporaryDirectories: string[] = [];

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

describe('renderer IPC', () => {
  it('converts only loopback ACP websocket URLs', () => {
    expect(loopbackHttpBaseFromAcpUrl('ws://127.0.0.1:3000/acp?secret=x')).toBe(
      'http://127.0.0.1:3000'
    );
    expect(loopbackHttpBaseFromAcpUrl('wss://localhost:3000/acp')).toBe('https://localhost:3000');
    expect(loopbackHttpBaseFromAcpUrl('ws://example.com/acp')).toBeNull();
    expect(loopbackHttpBaseFromAcpUrl('https://127.0.0.1/acp')).toBeNull();
  });

  it('grants a folder chosen for a session so its outputs preview without a second prompt', async () => {
    const on = vi.fn();
    const handle = vi.fn();
    const grantSelectedPath = vi.fn();
    registerRendererIpcHandlers(
      { on, handle },
      {
        log: { info: vi.fn(), error: vi.fn() },
        pendingInitialMessages: new Map(),
        pendingInitialMessageNoAutoSubmit: new Set(),
        pendingDeepLinks: new Map(),
        reactReadyWindows: new Set(),
        sendOpenSharedSession: vi.fn(),
        openExternalIfSafe: vi.fn(),
        rendererDirectoryGrants: { grantSelectedPath } as never,
        assertRendererFileAccess: vi.fn(),
        goslingServeLeases: {} as never,
      }
    );
    const chooser = handle.mock.calls.find(
      ([channel]) => channel === 'session-directory-chooser'
    )?.[1] as (event: { sender: { id: number } }) => Promise<unknown>;

    vi.mocked(dialog.showOpenDialog).mockResolvedValue({
      canceled: false,
      filePaths: ['/Users/tester/Work/research'],
    } as Awaited<ReturnType<typeof dialog.showOpenDialog>>);
    await chooser({ sender: { id: 7 } });
    expect(grantSelectedPath).toHaveBeenCalledWith(7, '/Users/tester/Work/research');

    grantSelectedPath.mockClear();
    vi.mocked(dialog.showOpenDialog).mockResolvedValue({
      canceled: true,
      filePaths: [],
    } as Awaited<ReturnType<typeof dialog.showOpenDialog>>);
    await chooser({ sender: { id: 7 } });
    expect(grantSelectedPath).not.toHaveBeenCalled();
  });

  it('grants a session its own directories, refusing anything too broad to be useful', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-session-grants-'));
    temporaryDirectories.push(root);
    const project = path.join(root, 'project');
    const missing = path.join(root, 'gone');
    const link = path.join(root, 'link');
    const file = path.join(root, 'notes.md');
    fs.mkdirSync(project);
    fs.writeFileSync(file, 'notes');
    fs.symlinkSync(project, link, 'dir');

    const handle = vi.fn();
    const grants = new RendererDirectoryGrantRegistry(path.join(root, 'grants.json'));
    registerRendererIpcHandlers(
      { on: vi.fn(), handle },
      {
        log: { info: vi.fn(), error: vi.fn() },
        pendingInitialMessages: new Map(),
        pendingInitialMessageNoAutoSubmit: new Set(),
        pendingDeepLinks: new Map(),
        reactReadyWindows: new Set(),
        sendOpenSharedSession: vi.fn(),
        openExternalIfSafe: vi.fn(),
        rendererDirectoryGrants: grants,
        assertRendererFileAccess: vi.fn(),
        goslingServeLeases: {} as never,
      }
    );
    const grant = handle.mock.calls.find(
      ([channel]) => channel === 'grant-session-directories'
    )?.[1] as (event: { sender: { id: number } }, dirs: unknown) => string[];

    const granted = grant({ sender: { id: 7 } }, [
      project,
      os.homedir(), // would subsume every other grant
      path.parse(root).root, // the whole filesystem
      link, // symlinked directory
      file, // not a directory
      missing, // no longer exists
      '',
    ]);

    expect(granted).toEqual([fs.realpathSync.native(project)]);
    expect(grants.isGrantedDirectory(7, project)).toBe(true);
    expect(grants.isGrantedDirectory(7, os.homedir())).toBe(false);
    // Nothing durable: the session's folders are re-granted on load, not remembered.
    expect(fs.existsSync(path.join(root, 'grants.json'))).toBe(false);

    expect(grant({ sender: { id: 7 } }, 'not-an-array')).toEqual([]);
    expect(grant({ sender: { id: 7 } }, new Array(65).fill(project))).toEqual([]);
  });

  it('registers renderer readiness and the original handler set', () => {
    const on = vi.fn();
    const handle = vi.fn();
    registerRendererIpcHandlers(
      { on, handle },
      {
        log: { info: vi.fn(), error: vi.fn() },
        pendingInitialMessages: new Map(),
        pendingInitialMessageNoAutoSubmit: new Set(),
        pendingDeepLinks: new Map(),
        reactReadyWindows: new Set(),
        sendOpenSharedSession: vi.fn(),
        openExternalIfSafe: vi.fn(),
        rendererDirectoryGrants: {} as never,
        assertRendererFileAccess: vi.fn(),
        goslingServeLeases: {} as never,
      }
    );
    expect(on.mock.calls.map(([channel]) => channel)).toEqual(RENDERER_IPC_ON_CHANNELS);
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(RENDERER_IPC_HANDLE_CHANNELS);
  });
});
