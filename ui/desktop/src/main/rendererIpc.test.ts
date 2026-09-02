import { describe, expect, it, vi } from 'vitest';
import { loopbackHttpBaseFromAcpUrl, registerRendererIpcHandlers } from './rendererIpc';

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
    expect(on).toHaveBeenCalledOnce();
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual([
      'open-external',
      'directory-chooser',
      'session-directory-chooser',
      'add-recent-dir',
      'list-recent-dirs',
      'list-git-worktree-dirs',
      'get-git-branch-info',
      'list-git-branches',
      'switch-git-branch',
      'get-acp-url',
      'get-mcp-app-proxy-url',
    ]);
  });
});
