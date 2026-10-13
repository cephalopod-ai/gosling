// @vitest-environment node
import { execFile } from 'node:child_process';
import { dialog } from 'electron';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { describe, expect, it, vi } from 'vitest';
import {
  ensureGitignoredIfInRepo,
  registerSettingsIpcHandlers,
  SETTINGS_IPC_CHANNELS,
} from './settingsIpc';
import { desktopCommandChannels } from '../ipc/channels';
import { defaultSettings } from '../utils/settings';

vi.mock('electron', () => ({
  dialog: { showOpenDialog: vi.fn(), showMessageBox: vi.fn(async () => ({ response: 1 })) },
}));

describe('settings IPC registration', () => {
  it('registers every original settings and research channel once', () => {
    const handle = vi.fn();
    registerSettingsIpcHandlers(
      { handle },
      {
        app: {} as never,
        getSettings: vi.fn(),
        updateSettings: vi.fn(),
        getExternalBackendSecret: vi.fn(),
        setExternalBackendSecret: vi.fn(),
        updateConfiguredLocale: vi.fn(),
        registerGlobalShortcuts: vi.fn(),
        setAutoDownloadDisabled: vi.fn(),
        rendererDirectoryGrants: {} as never,
      }
    );
    expect(handle.mock.calls.map(([channel]) => channel)).toEqual(SETTINGS_IPC_CHANNELS);
  });

  it('keeps the default research library inside an isolated Gosling path root', async () => {
    const pathRoot = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-isolated-root-'));
    const handlers = new Map<string, (...args: never[]) => unknown>();
    const grantSelectedPath = vi.fn();

    registerSettingsIpcHandlers(
      {
        handle: vi.fn((channel, handler) => {
          handlers.set(channel, handler as (...args: never[]) => unknown);
        }),
      },
      {
        app: { getPath: vi.fn(() => '/Users/operator/Documents') } as never,
        goslingPathRoot: pathRoot,
        getSettings: () => ({ ...defaultSettings }),
        updateSettings: vi.fn(),
        getExternalBackendSecret: vi.fn(() => ''),
        setExternalBackendSecret: vi.fn(),
        updateConfiguredLocale: vi.fn(),
        registerGlobalShortcuts: vi.fn(),
        setAutoDownloadDisabled: vi.fn(),
        rendererDirectoryGrants: { grantSelectedPath } as never,
      }
    );

    const getResearchLibraryPath = handlers.get(desktopCommandChannels.getResearchLibraryPath);
    const libraryPath = await getResearchLibraryPath?.({ sender: { id: 17 } } as never);

    expect(libraryPath).toBe(path.join(pathRoot, 'Gosling Research Library'));
    expect(grantSelectedPath).toHaveBeenCalledWith(17, libraryPath);
    await expect(fs.stat(libraryPath as string)).resolves.toMatchObject({});
  });
});

describe('ensureGitignoredIfInRepo', () => {
  const runGit = promisify(execFile);

  async function makeRepo() {
    const repo = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-gitignore-test-'));
    await runGit('git', ['-C', repo, 'init', '--quiet']);
    return repo;
  }

  it('adds a nested library folder to a .gitignore beside it', async () => {
    const repo = await makeRepo();
    const libraryPath = path.join(repo, 'Library');
    await fs.mkdir(libraryPath);

    await ensureGitignoredIfInRepo(libraryPath);

    const gitignore = await fs.readFile(path.join(repo, '.gitignore'), 'utf8');
    expect(gitignore).toBe('/Library/\n');
  });

  it('does not duplicate the entry on repeated calls', async () => {
    const repo = await makeRepo();
    const libraryPath = path.join(repo, 'Library');
    await fs.mkdir(libraryPath);

    await ensureGitignoredIfInRepo(libraryPath);
    await ensureGitignoredIfInRepo(libraryPath);

    const gitignore = await fs.readFile(path.join(repo, '.gitignore'), 'utf8');
    expect(gitignore).toBe('/Library/\n');
  });

  it('preserves existing rules under concurrent additions', async () => {
    const repo = await makeRepo();
    await fs.writeFile(path.join(repo, '.gitignore'), 'existing-rule');
    const first = path.join(repo, 'Library');
    const second = path.join(repo, 'Other');
    await fs.mkdir(first);
    await fs.mkdir(second);
    await Promise.all([ensureGitignoredIfInRepo(first), ensureGitignoredIfInRepo(second)]);
    const rules = await fs.readFile(path.join(repo, '.gitignore'), 'utf8');
    expect(rules.startsWith('existing-rule\n')).toBe(true);
    expect(rules).toContain('/Library/\n');
    expect(rules).toContain('/Other/\n');
  });

  it('leaves ignore rules unchanged when native confirmation is canceled', async () => {
    const repo = await makeRepo();
    const library = path.join(repo, 'Library');
    await fs.mkdir(library);
    vi.mocked(dialog.showMessageBox).mockResolvedValueOnce({ response: 0, checkboxChecked: false });
    await ensureGitignoredIfInRepo(library);
    await expect(fs.access(path.join(repo, '.gitignore'))).rejects.toThrow();
  });

  it('leaves no .gitignore behind outside a git repository', async () => {
    const plainDir = await fs.mkdtemp(path.join(os.tmpdir(), 'gosling-no-repo-test-'));
    const libraryPath = path.join(plainDir, 'Library');
    await fs.mkdir(libraryPath);

    await ensureGitignoredIfInRepo(libraryPath);

    await expect(fs.access(path.join(plainDir, '.gitignore'))).rejects.toThrow();
  });
});
