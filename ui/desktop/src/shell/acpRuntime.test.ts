import { describe, expect, it, vi } from 'vitest';
import type { GoslingClientCallbacks } from '@repo-makeover/gosling-sdk';
import type { InitializeResponse } from '@agentclientprotocol/sdk';
import { connectShellAcp, ShellCompatibilityError } from './acpRuntime';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';

const methods = [
  '_gosling/unstable/shell/handoff/prepare',
  '_gosling/unstable/shell/provisioning/read',
  '_gosling/unstable/shell/provisioning/validate',
];
const product = {
  id: 'gosling-shell-fixture-a',
  displayName: 'Gosling Shell Fixture A',
  version: '0.0.0-test',
  runtimeNamespace: 'shell-fixture-a',
  protocolScheme: 'gosling-fixture-a',
  executableName: 'gosling-shell-fixture-a',
  macosBundleId: 'io.github.repo-makeover.gosling.fixture.a',
  windowsAppId: 'Gosling.Shell.Fixture.A',
  linuxPackageName: 'gosling-shell-fixture-a',
  flatpakId: 'io.github.repo_makeover.Gosling.FixtureA',
};
const profile: ResolvedShellProductProfile = {
  schemaVersion: 1,
  product,
  provisioningPath: 'fixtures/provisioning.json',
  compatibility: {
    goslingVersion: '0.1.0',
    goslingRevision: 'current',
    provisioningSchemaVersion: 1,
    handoffSchemaVersion: 1,
    requiredMethods: methods,
  },
  assets: {
    root: 'fixtures/assets',
    iconBase: 'fixtures/assets/icon',
    requiredTargets: ['linux-x64'],
  },
  update: { enabled: false, channel: 'fixture-disabled' },
  distribution: { publishable: false, artifactPrefix: 'fixture-a', signingPolicy: 'none' },
};
const manifest: ShellBuildManifest = {
  schemaVersion: 1,
  profileSchemaVersion: 1,
  profileHash: 'a'.repeat(64),
  product,
  target: 'linux-x64',
  platform: 'linux',
  architecture: 'x64',
  sourceClean: false,
  compatibility: {
    goslingVersion: '0.1.0',
    goslingRevision: 'b'.repeat(40),
    provisioningSchemaVersion: 1,
    handoffSchemaVersion: 1,
    requiredMethods: methods,
  },
};

function response(): InitializeResponse {
  return {
    protocolVersion: 1,
    agentInfo: { name: 'gosling', version: '0.1.0' },
    agentCapabilities: {
      _meta: {
        goslingShell: {
          identity: {
            id: product.id,
            displayName: product.displayName,
            version: product.version,
          },
          availableMethods: methods,
        },
      },
    },
  };
}

function harness() {
  const close = vi.fn();
  const initialize = vi.fn(() => Promise.resolve(response()));
  const read = vi.fn(() =>
    Promise.resolve({
      provisioning: {
        schemaVersion: 1,
        identity: {
          id: product.id,
          displayName: product.displayName,
          version: product.version,
        },
      },
      validation: { valid: true, issues: [] },
    })
  );
  const validate = vi.fn((request) =>
    Promise.resolve({ provisioning: request.provisioning, validation: { valid: true, issues: [] } })
  );
  const prepare = vi.fn(() => Promise.reject(new Error('not used')));
  const client = {
    signal: new AbortController().signal,
    closed: new Promise<void>(() => {}),
    initialize,
    gosling: {
      shellProvisioningRead_unstable: read,
      shellProvisioningValidate_unstable: validate,
      shellHandoffPrepare_unstable: prepare,
    },
  };
  const createStream = vi.fn(() => ({
    readable: new globalThis.ReadableStream(),
    writable: new globalThis.WritableStream(),
    close,
  }));
  const createClient = vi.fn((callbacks: () => GoslingClientCallbacks) => {
    void callbacks;
    return client;
  });
  const dependencies = {
    createStream,
    createClient,
    setTimeout,
    clearTimeout,
  };
  return { client, close, createClient, createStream, dependencies, initialize, read, validate };
}

function connect(value = harness()) {
  return {
    value,
    promise: connectShellAcp({
      acpUrl: 'ws://127.0.0.1:7777/acp?token=private',
      profile,
      manifest,
      clientName: 'fixture-shell',
      clientVersion: '0.0.0-test',
      dependencies: value.dependencies,
    }),
  };
}

describe('shell ACP runtime', () => {
  it('initializes, reads, validates, and checks compatibility without creating a session', async () => {
    const { promise, value } = connect();
    const connection = await promise;
    expect(value.createStream).toHaveBeenCalledWith('ws://127.0.0.1:7777/acp?token=private');
    expect(value.initialize).toHaveBeenCalledWith({
      protocolVersion: 1,
      clientCapabilities: {},
      clientInfo: { name: 'fixture-shell', version: '0.0.0-test' },
    });
    expect(value.read).toHaveBeenCalledWith({});
    expect(value.validate).toHaveBeenCalledTimes(1);
    expect(Object.keys(value.client)).not.toContain('newSession');
    const callbacks = value.createClient.mock.calls[0][0]();
    expect(Object.keys(callbacks).sort()).toEqual([
      'requestPermission',
      'sessionUpdate',
      'unstable_createElicitation',
    ]);
    expect(await callbacks.requestPermission({} as never)).toEqual({
      outcome: { outcome: 'cancelled' },
    });
    expect(await callbacks.unstable_createElicitation!({} as never)).toEqual({ action: 'decline' });
    expect(connection.compatibility).toEqual({ compatible: true });
    expect(value.close).not.toHaveBeenCalled();
    connection.close();
    expect(value.close).toHaveBeenCalledOnce();
  });

  it.each([
    'ws://localhost:7777/acp?token=private',
    'https://127.0.0.1:7777/acp?token=private',
    'ws://127.0.0.1:7777/other?token=private',
    'ws://127.0.0.1:7777/acp',
    'ws://user:pass@127.0.0.1:7777/acp?token=private',
    'ws://127.0.0.1:7777/acp?token=private&extra=value',
  ])('rejects non-canonical ACP endpoint %s before creating a transport', async (acpUrl) => {
    const value = harness();
    await expect(
      connectShellAcp({
        acpUrl,
        profile,
        manifest,
        clientName: 'fixture-shell',
        clientVersion: '0.0.0-test',
        dependencies: value.dependencies,
      })
    ).rejects.toThrow(/ACP endpoint/);
    expect(value.createStream).not.toHaveBeenCalled();
  });

  it('fails closed and closes transport when initialization metadata is absent', async () => {
    const value = harness();
    value.initialize.mockResolvedValue({ protocolVersion: 1 });
    await expect(connect(value).promise).rejects.toThrow('omitted shell capability metadata');
    expect(value.read).not.toHaveBeenCalled();
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('rejects missing methods before calling provisioning', async () => {
    const value = harness();
    const incompatible = response();
    const shell = incompatible.agentCapabilities!._meta!.goslingShell as {
      identity: { id: string };
      availableMethods: string[];
    };
    shell.availableMethods = shell.availableMethods.slice(0, -1);
    value.initialize.mockResolvedValue(incompatible);
    await expect(connect(value).promise).rejects.toMatchObject({
      result: { compatible: false, code: 'METHOD_UNAVAILABLE' },
    });
    expect(value.read).not.toHaveBeenCalled();
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('rejects runtime identity after provisioning confirms the fixed identity', async () => {
    const value = harness();
    const incompatible = response();
    const shell = incompatible.agentCapabilities!._meta!.goslingShell as {
      identity: { id: string };
      availableMethods: string[];
    };
    shell.identity.id = 'other';
    value.initialize.mockResolvedValue(incompatible);
    await expect(connect(value).promise).rejects.toBeInstanceOf(ShellCompatibilityError);
    expect(value.read).toHaveBeenCalledOnce();
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('fails closed when either server validation report is invalid', async () => {
    const value = harness();
    value.read.mockResolvedValue({
      provisioning: {
        schemaVersion: 1,
        identity: {
          id: product.id,
          displayName: product.displayName,
          version: product.version,
        },
      },
      validation: { valid: false, issues: [] },
    });
    await expect(connect(value).promise).rejects.toMatchObject({
      result: { compatible: false, code: 'PROVISIONING_INVALID' },
    });
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('closes transport when provisioning transport or schema parsing fails', async () => {
    const value = harness();
    value.read.mockRejectedValue(new Error('read failed'));
    await expect(connect(value).promise).rejects.toThrow('read failed');
    expect(value.validate).not.toHaveBeenCalled();
    expect(value.close).toHaveBeenCalledOnce();
  });
});
