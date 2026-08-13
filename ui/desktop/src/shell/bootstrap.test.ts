import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';

const controller = vi.hoisted(() => {
  const state = {
    generation: 1,
    name: 'booting' as const,
    enteredAt: 'now',
    allowedActions: ['stop' as const],
  };
  return {
    value: {
      read: vi.fn(() => state),
      start: vi.fn(async () => state),
      retry: vi.fn(async () => state),
      stop: vi.fn(async () => ({ ...state, name: 'stopped' as const, allowedActions: [] })),
      onChanged: vi.fn(() => vi.fn()),
      getAcp: vi.fn(() => null),
      getStartupDiagnostics: vi.fn(() => null),
      getExitDetails: vi.fn(() => null),
    },
    create: vi.fn(),
  };
});
controller.create.mockReturnValue(controller.value);
vi.mock('./runtimeController', () => ({ createShellRuntimeController: controller.create }));

import type Electron from 'electron';
import type { ShellBootstrapAdapter } from './bootstrap';
import { bootstrapShell } from './bootstrap';
import { shellIpcChannels } from './ipc';

const roots: string[] = [];
afterEach(() => {
  vi.clearAllMocks();
  controller.create.mockReturnValue(controller.value);
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true });
});

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.entries(value)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, child]) => `${JSON.stringify(key)}:${canonicalJson(child)}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function resources() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-bootstrap-'));
  roots.push(root);
  const product = {
    id: 'fixture',
    displayName: 'Fixture',
    version: '0.0.0-test',
    runtimeNamespace: 'fixture-runtime',
    protocolScheme: 'fixture',
    executableName: 'fixture',
    macosBundleId: 'test.fixture',
    windowsAppId: 'Test.Fixture',
    linuxPackageName: 'test-fixture',
    flatpakId: 'test.Fixture',
  };
  const profile = {
    schemaVersion: 1,
    product,
    provisioningPath: 'provisioning.json',
    compatibility: {
      goslingVersion: '0.1.0',
      goslingRevision: 'current',
      provisioningSchemaVersion: 1,
      handoffSchemaVersion: 1,
      requiredMethods: [],
    },
    assets: { root: 'assets', iconBase: 'assets/icon', requiredTargets: ['linux-x64'] },
    update: { enabled: false, channel: 'disabled' },
    distribution: { publishable: false, artifactPrefix: 'fixture', signingPolicy: 'none' },
  };
  const manifest = {
    schemaVersion: 1,
    profileSchemaVersion: 1,
    profileHash: crypto.createHash('sha256').update(canonicalJson(profile)).digest('hex'),
    product,
    target: 'linux-x64',
    platform: 'linux',
    architecture: 'x64',
    sourceClean: false,
    compatibility: {
      goslingVersion: '0.1.0',
      goslingRevision: 'a'.repeat(40),
      provisioningSchemaVersion: 1,
      handoffSchemaVersion: 1,
      requiredMethods: [],
    },
  };
  for (const [name, value] of [
    ['profile.json', profile],
    ['manifest.json', manifest],
    ['provisioning.json', {}],
  ] as const) {
    fs.writeFileSync(path.join(root, name), JSON.stringify(value));
  }
  return {
    root,
    product,
    files: {
      profileFileName: 'profile.json',
      manifestFileName: 'manifest.json',
      provisioningFileName: 'provisioning.json',
      developmentProfilePath: path.join(root, 'profile.json'),
      developmentManifestPath: path.join(root, 'manifest.json'),
      developmentProvisioningPath: path.join(root, 'provisioning.json'),
    },
  };
}

function harness(lock = true) {
  const value = resources();
  const calls: string[] = [];
  const appListeners = new Map<string, (...args: unknown[]) => void>();
  const windowListeners = new Map<string, () => void>();
  const webListeners = new Map<string, (...args: unknown[]) => void>();
  const handlers = new Map<string, Parameters<ShellBootstrapAdapter['ipcMain']['handle']>[1]>();
  const mainFrame = {};
  const webContents = {
    id: 1,
    mainFrame,
    send: vi.fn(),
    setWindowOpenHandler: vi.fn(),
    on: vi.fn((event: string, listener: (...args: unknown[]) => void) =>
      webListeners.set(event, listener)
    ),
  };
  const window = {
    webContents,
    once: vi.fn((event: string, listener: () => void) => windowListeners.set(event, listener)),
    on: vi.fn((event: string, listener: () => void) => windowListeners.set(event, listener)),
    isDestroyed: vi.fn(() => false),
    isMinimized: vi.fn(() => false),
    restore: vi.fn(),
    show: vi.fn(),
    focus: vi.fn(),
    loadURL: vi.fn(async () => {}),
  };
  const app = {
    isPackaged: true,
    getPath: vi.fn((name: string) => {
      calls.push(`get:${name}`);
      return path.join(value.root, name);
    }),
    setName: vi.fn(() => calls.push('setName')),
    setPath: vi.fn((name: string) => calls.push(`setPath:${name}`)),
    requestSingleInstanceLock: vi.fn(() => {
      calls.push('lock');
      return lock;
    }),
    setAsDefaultProtocolClient: vi.fn(() => {
      calls.push('protocol');
      return true;
    }),
    whenReady: vi.fn(async () => calls.push('ready')),
    on: vi.fn((event: string, listener: (...args: unknown[]) => void) =>
      appListeners.set(event, listener)
    ),
    quit: vi.fn(),
    exit: vi.fn(),
  };
  const adapter = {
    app,
    ipcMain: {
      handle: vi.fn(
        (channel: string, handler: Parameters<ShellBootstrapAdapter['ipcMain']['handle']>[1]) =>
          handlers.set(channel, handler)
      ),
      removeHandler: vi.fn((channel: string) => handlers.delete(channel)),
    },
    createWindow: vi.fn(() => window),
    showSaveDialog: vi.fn(async () => ({ canceled: true })),
    openExternal: vi.fn(async () => {}),
    resourcesPath: value.root,
    preloadPath: '/app/shell-preload.js',
    rendererUrl: 'file:///app/shell/index.html',
    workingDir: '/workspace',
    shellResources: value.files,
    now: () => 'now',
  };
  return {
    adapter,
    app,
    appListeners,
    calls,
    handlers,
    mainFrame,
    value,
    webListeners,
    window,
    windowListeners,
  };
}

describe('shell bootstrap', () => {
  it('applies identity before lock/readiness and creates only the dedicated secure window', async () => {
    const value = harness();
    const result = await bootstrapShell(value.adapter as never);
    expect(result.started).toBe(true);
    expect(value.calls.indexOf('setName')).toBeLessThan(value.calls.indexOf('lock'));
    expect(value.calls.indexOf('setPath:userData')).toBeLessThan(value.calls.indexOf('lock'));
    expect(value.calls.indexOf('lock')).toBeLessThan(value.calls.indexOf('ready'));
    expect(value.adapter.createWindow).toHaveBeenCalledWith(
      expect.objectContaining({
        webPreferences: expect.objectContaining({
          preload: '/app/shell-preload.js',
          partition: 'persist:gosling-shell-fixture',
          sandbox: true,
          nodeIntegration: false,
          contextIsolation: true,
        }),
      })
    );
    expect(value.window.loadURL).toHaveBeenCalledWith(value.adapter.rendererUrl);
    expect(controller.value.start).toHaveBeenCalledOnce();
  });

  it('quits before readiness, window, IPC, or runtime when lock is unavailable', async () => {
    const value = harness(false);
    const result = await bootstrapShell(value.adapter as never);
    expect(result.started).toBe(false);
    expect(value.app.quit).toHaveBeenCalledOnce();
    expect(value.app.whenReady).not.toHaveBeenCalled();
    expect(value.adapter.createWindow).not.toHaveBeenCalled();
    expect(controller.value.start).not.toHaveBeenCalled();
  });

  it('denies new windows and navigation away from the fixed renderer URL', async () => {
    const value = harness();
    await bootstrapShell(value.adapter as never);
    expect(
      value.window.webContents.setWindowOpenHandler.mock.calls[0][0]({ url: 'https://evil' })
    ).toEqual({
      action: 'deny',
    });
    const preventDefault = vi.fn();
    value.webListeners.get('will-navigate')!({ preventDefault }, 'https://evil');
    expect(preventDefault).toHaveBeenCalledOnce();
  });

  it('registers the exact IPC set and delegates runtime read without URL or secret', async () => {
    const value = harness();
    await bootstrapShell(value.adapter as never);
    expect([...value.handlers.keys()].sort()).toEqual(
      Object.values(shellIpcChannels)
        .filter((channel) => channel !== shellIpcChannels.runtimeChanged)
        .sort()
    );
    const response = await value.handlers.get(shellIpcChannels.runtimeRead)!({
      sender: value.window.webContents as unknown as Electron.WebContents,
      senderFrame: value.mainFrame as Electron.WebFrameMain,
    });
    expect(response).toEqual(controller.value.read());
    expect(JSON.stringify(response)).not.toMatch(/token|secret|acp/i);
  });

  it('focuses a second instance and waits for runtime cleanup before final quit', async () => {
    const value = harness();
    await bootstrapShell(value.adapter as never);
    value.appListeners.get('second-instance')!();
    expect(value.window.show).toHaveBeenCalled();
    expect(value.window.focus).toHaveBeenCalledOnce();

    const event = { preventDefault: vi.fn() };
    value.appListeners.get('before-quit')!(event);
    expect(event.preventDefault).toHaveBeenCalledOnce();
    await vi.waitFor(() => expect(controller.value.stop).toHaveBeenCalledWith(1));
    await vi.waitFor(() => expect(value.app.quit).toHaveBeenCalledOnce());
  });
});
