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
      onSessionUpdated: vi.fn(() => vi.fn()),
      onInteractionRequested: vi.fn(() => vi.fn()),
      getSessionController: vi.fn(() => null),
      getInteractionController: vi.fn(() => null),
      getAcp: vi.fn(() => null),
      getStartupDiagnostics: vi.fn(() => null),
      getExitDetails: vi.fn(() => null),
    },
    create: vi.fn(),
  };
});
controller.create.mockReturnValue(controller.value);
vi.mock('./runtimeController', () => ({ createShellRuntimeController: controller.create }));

const directoryController = vi.hoisted(() => ({
  read: vi.fn(() => ({
    state: 'unselected',
    path: null,
    label: null,
    reasonCode: null,
    remembered: false,
  })),
  accepted: vi.fn(() => null),
  restore: vi.fn(),
  select: vi.fn(),
  clear: vi.fn(),
  onChanged: vi.fn(() => vi.fn()),
}));
vi.mock('./directoryController', () => ({
  createShellDirectoryController: vi.fn(() => directoryController),
}));

const credentialController = vi.hoisted(() => ({
  read: vi.fn(() => ({
    catalogStatus: 'denied',
    profiles: [],
    selectedProfileId: null,
    selectionStatus: 'none',
  })),
  selected: vi.fn(() => null),
  refresh: vi.fn(),
  select: vi.fn(),
  clear: vi.fn(),
  onChanged: vi.fn(() => vi.fn()),
}));
vi.mock('./credentialController', () => ({
  createShellCredentialController: vi.fn(() => credentialController),
}));

import type Electron from 'electron';
import type { ShellBootstrapAdapter } from './bootstrap';
import { bootstrapShell } from './bootstrap';
import { shellIpcChannels } from './ipc';
import { decodeShellOperationFailure } from './operationFailure';

async function expectFailure(pending: Promise<unknown>, code: string): Promise<void> {
  try {
    await pending;
  } catch (error) {
    expect(decodeShellOperationFailure(error)).toMatchObject({ code });
    return;
  }
  throw new Error(`expected ${code}`);
}

const roots: string[] = [];
afterEach(() => {
  vi.clearAllMocks();
  controller.create.mockReturnValue(controller.value);
  controller.value.getAcp.mockReturnValue(null);
  controller.value.getSessionController.mockReturnValue(null);
  controller.value.getInteractionController.mockReturnValue(null);
  directoryController.accepted.mockReturnValue(null);
  credentialController.selected.mockReturnValue(null);
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
    isDestroyed: vi.fn(() => false),
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
    showConfirmDialog: vi.fn(async () => ({ confirmed: true })),
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
        .filter((channel) => channel !== shellIpcChannels.sessionUpdated)
        .filter((channel) => channel !== shellIpcChannels.permissionRequested)
        .filter((channel) => channel !== shellIpcChannels.elicitationRequested)
        .filter((channel) => channel !== shellIpcChannels.confirmationRequested)
        .sort()
    );
    const response = await value.handlers.get(shellIpcChannels.runtimeRead)!({
      sender: value.window.webContents as unknown as Electron.WebContents,
      senderFrame: value.mainFrame as Electron.WebFrameMain,
    });
    expect(response).toEqual(controller.value.read());
    expect(JSON.stringify(response)).not.toMatch(/token|secret|acp/i);
  });

  it('reads and updates real product-local settings through the store, never a raw file', async () => {
    const value = harness();
    await bootstrapShell(value.adapter as never);
    const event = {
      sender: value.window.webContents as unknown as Electron.WebContents,
      senderFrame: value.mainFrame as Electron.WebFrameMain,
    };

    const initial = await value.handlers.get(shellIpcChannels.settingsRead)!(event);
    expect(initial).toEqual({
      appearance: { theme: 'system', textScale: 1 },
      recovery: { status: 'absent', schemaVersion: null },
    });

    const updated = await value.handlers.get(shellIpcChannels.settingsAppearanceUpdate)!(event, {
      generation: 1,
      theme: 'dark',
    });
    expect(updated).toEqual({
      appearance: { theme: 'dark', textScale: 1 },
      recovery: { status: 'loaded', schemaVersion: 1 },
    });
    const reread = await value.handlers.get(shellIpcChannels.settingsRead)!(event);
    expect(reread).toEqual(updated);
  });

  it('resets settings only after explicit confirmation, and leaves them untouched on cancel', async () => {
    const value = harness();
    await bootstrapShell(value.adapter as never);
    const event = {
      sender: value.window.webContents as unknown as Electron.WebContents,
      senderFrame: value.mainFrame as Electron.WebFrameMain,
    };
    await value.handlers.get(shellIpcChannels.settingsAppearanceUpdate)!(event, {
      generation: 1,
      theme: 'dark',
      textScale: 1.5,
    });

    value.adapter.showConfirmDialog.mockResolvedValueOnce({ confirmed: false });
    const cancelled = await value.handlers.get(shellIpcChannels.settingsReset)!(event, {
      generation: 1,
      userGesture: true,
    });
    expect(cancelled).toEqual({
      appearance: { theme: 'dark', textScale: 1.5 },
      recovery: { status: 'loaded', schemaVersion: 1 },
    });

    value.adapter.showConfirmDialog.mockResolvedValueOnce({ confirmed: true });
    const reset = await value.handlers.get(shellIpcChannels.settingsReset)!(event, {
      generation: 1,
      userGesture: true,
    });
    expect(reset).toEqual({
      appearance: { theme: 'system', textScale: 1 },
      recovery: { status: 'loaded', schemaVersion: 1 },
    });
    expect(value.adapter.showConfirmDialog).toHaveBeenCalledWith(
      expect.objectContaining({ title: 'Reset local settings' })
    );
  });

  it('clears the live directory and credential selection on a confirmed reset, not on cancel', async () => {
    const value = harness();
    await bootstrapShell(value.adapter as never);
    const event = {
      sender: value.window.webContents as unknown as Electron.WebContents,
      senderFrame: value.mainFrame as Electron.WebFrameMain,
    };

    value.adapter.showConfirmDialog.mockResolvedValueOnce({ confirmed: false });
    await value.handlers.get(shellIpcChannels.settingsReset)!(event, {
      generation: 1,
      userGesture: true,
    });
    expect(directoryController.clear).not.toHaveBeenCalled();
    expect(credentialController.clear).not.toHaveBeenCalled();

    value.adapter.showConfirmDialog.mockResolvedValueOnce({ confirmed: true });
    await value.handlers.get(shellIpcChannels.settingsReset)!(event, {
      generation: 1,
      userGesture: true,
    });
    expect(directoryController.clear).toHaveBeenCalledTimes(1);
    expect(credentialController.clear).toHaveBeenCalledTimes(1);
  });

  it('rejects settings mutations carrying a stale generation, like every other mutating channel', async () => {
    const value = harness();
    await bootstrapShell(value.adapter as never);
    const event = {
      sender: value.window.webContents as unknown as Electron.WebContents,
      senderFrame: value.mainFrame as Electron.WebFrameMain,
    };

    await expectFailure(
      Promise.resolve(
        value.handlers.get(shellIpcChannels.settingsAppearanceUpdate)!(event, {
          generation: 2,
          theme: 'dark',
        })
      ),
      'STALE_REQUEST'
    );
    await expectFailure(
      Promise.resolve(
        value.handlers.get(shellIpcChannels.settingsReset)!(event, {
          generation: 2,
          userGesture: true,
        })
      ),
      'STALE_REQUEST'
    );
    expect(value.adapter.showConfirmDialog).not.toHaveBeenCalled();
  });

  it('relays domain reads, actions, and confirmation through the verified active session only', async () => {
    const snapshot = vi.fn(async () => ({
      domainId: 'neutral-fixture',
      payload: {},
      resources: [],
    }));
    const action = vi.fn(async () => ({
      domainId: 'neutral-fixture',
      action: 'toggle',
      confirmationActionId: 'confirm-a',
    }));
    const confirm = vi.fn(async () => ({ status: 'denied' as const }));
    controller.value.getAcp.mockReturnValue({
      domainAdapter: {
        descriptorId: 'neutral-fixture',
        protocolVersion: '1.0.0',
        actions: ['toggle'],
      },
      domainSnapshot: snapshot,
      domainAction: action,
      confirmDomainAction: confirm,
    } as never);
    controller.value.getSessionController.mockReturnValue({
      read: () => ({ sessionId: 'session-a', status: 'active' }),
    } as never);
    const confirmations = new Map<string, Record<string, unknown>>();
    const interactions = {
      requestConfirmation: vi.fn((input: Record<string, unknown> & { actionId: string }) => {
        confirmations.set(input.actionId, input);
      }),
      read: vi.fn(() =>
        [...confirmations.values()].map((input) => ({ ...input, kind: 'confirm' }))
      ),
      respondConfirmation: vi.fn((input: { actionId: string }) =>
        confirmations.delete(input.actionId)
      ),
    };
    controller.value.getInteractionController.mockReturnValue(interactions as never);
    const value = harness();
    const manifestPath = path.join(value.value.root, 'manifest.json');
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    manifest.consumer = {
      consumerId: 'fixture-consumer',
      consumerHash: 'c'.repeat(64),
      rendererHash: 'd'.repeat(64),
      declaredCapabilities: ['domain.snapshot', 'domain.action', 'confirmation.respond'],
      requiredAgentCapabilities: [],
      requiredMethods: [],
    };
    fs.writeFileSync(manifestPath, JSON.stringify(manifest));
    await bootstrapShell(value.adapter as never);
    const event = {
      sender: value.window.webContents as unknown as Electron.WebContents,
      senderFrame: value.mainFrame as Electron.WebFrameMain,
    };

    await value.handlers.get(shellIpcChannels.domainSnapshot)!(event, {
      generation: 1,
      input: { scope: 'neutral' },
    });
    await value.handlers.get(shellIpcChannels.domainAction)!(event, {
      generation: 1,
      sessionId: 'session-a',
      action: 'toggle',
      input: { enabled: true },
    });
    await value.handlers.get(shellIpcChannels.confirmationRespond)!(event, {
      generation: 1,
      sessionId: 'session-a',
      actionId: 'confirm-a',
      approve: false,
    });
    expect(snapshot).toHaveBeenCalledWith({ input: { scope: 'neutral' } });
    expect(action).toHaveBeenCalledWith({
      sessionId: 'session-a',
      generation: 1,
      action: 'toggle',
      input: { enabled: true },
    });
    expect(confirm).toHaveBeenCalledWith({
      sessionId: 'session-a',
      generation: 1,
      actionId: 'confirm-a',
      approve: false,
    });
    await expectFailure(
      Promise.resolve(
        value.handlers.get(shellIpcChannels.domainAction)!(event, {
          generation: 2,
          sessionId: 'session-a',
          action: 'toggle',
        })
      ),
      'STALE_REQUEST'
    );
    await expectFailure(
      Promise.resolve(
        value.handlers.get(shellIpcChannels.confirmationRespond)!(event, {
          generation: 1,
          sessionId: 'foreign-session',
          actionId: 'confirm-a',
          approve: false,
        })
      ),
      'STALE_REQUEST'
    );
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
