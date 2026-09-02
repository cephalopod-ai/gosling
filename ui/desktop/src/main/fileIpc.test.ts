import { describe, expect, it, vi } from 'vitest';
import { FILE_IPC_CHANNELS, registerFileIpcHandlers } from './fileIpc';

vi.mock('electron', () => ({
  clipboard: { write: vi.fn(), writeText: vi.fn() },
  dialog: { showMessageBox: vi.fn(), showOpenDialog: vi.fn(), showSaveDialog: vi.fn() },
  shell: { openPath: vi.fn(), showItemInFolder: vi.fn() },
}));

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
});
