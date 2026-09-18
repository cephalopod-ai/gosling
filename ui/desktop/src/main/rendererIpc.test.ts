import { dialog } from 'electron';
import { describe, expect, it, vi } from 'vitest';
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
