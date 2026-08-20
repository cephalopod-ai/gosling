import { describe, expect, it, vi } from 'vitest';
import type {
  DomainActionConfirmResponse_unstable,
  DomainActionResponse_unstable,
  DomainSnapshotResponse_unstable,
  GoslingClientCallbacks,
  ShellLibraryResolveResponse_unstable,
} from '@repo-makeover/gosling-sdk';
import type { InitializeResponse } from '@agentclientprotocol/sdk';
import { connectShellAcp, provisioningIssueSummaries, ShellCompatibilityError } from './acpRuntime';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';

const sessionInfoMethod = '_gosling/unstable/session/info';
const mainOwnedMethods = [
  '_gosling/unstable/shell/credentials/list',
  '_gosling/unstable/shell/directory/validate',
  '_gosling/unstable/shell/modules/list',
];
const methods = [
  sessionInfoMethod,
  '_gosling/unstable/shell/handoff/prepare',
  '_gosling/unstable/shell/provisioning/read',
  '_gosling/unstable/shell/provisioning/validate',
  ...mainOwnedMethods,
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
      loadSession: true,
      sessionCapabilities: { list: {} },
      _meta: {
        goslingShell: {
          identity: {
            id: product.id,
            displayName: product.displayName,
            version: product.version,
            runtimeNamespace: product.runtimeNamespace,
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
          runtimeNamespace: product.runtimeNamespace,
        },
      },
      validation: { valid: true, issues: [] },
    })
  );
  const validate = vi.fn(() =>
    Promise.resolve({
      provisioning: {
        schemaVersion: 1,
        identity: {
          id: product.id,
          displayName: product.displayName,
          version: product.version,
          runtimeNamespace: product.runtimeNamespace,
        },
      },
      validation: { valid: true, issues: [] },
    })
  );
  const newSession = vi.fn(() => Promise.resolve({ sessionId: 'session-1' }));
  const loadSession = vi.fn(() => Promise.resolve({}));
  const listSessions = vi.fn(() => Promise.resolve({ sessions: [] }));
  const prompt = vi.fn(() => Promise.resolve({ stopReason: 'end_turn' as const }));
  const cancel = vi.fn(() => Promise.resolve());
  const sessionInfo = vi.fn(() =>
    Promise.resolve({
      session: {
        sessionId: 'session-1',
        cwd: '/workspace/saved',
        _meta: { gosling: { resumeIntegrity: 'clean' } },
      },
    })
  );
  const validateDirectory = vi.fn((params: { path: string }) =>
    Promise.resolve({ status: 'valid' as const, canonicalPath: params.path })
  );
  const listCredentials = vi.fn(() => Promise.resolve({ status: 'denied' as const, profiles: [] }));
  const listModules = vi.fn(() => Promise.resolve({ contractVersion: 1, modules: [] }));
  const listArtifacts = vi.fn(() =>
    Promise.resolve({ artifacts: [], totalCount: 0, truncated: false })
  );
  const listAvailableExtensions = vi.fn(() => Promise.resolve({ extensions: [] }));
  const listSessionExtensions = vi.fn(() => Promise.resolve({ extensions: [] }));
  const addSessionExtension = vi.fn(() => Promise.resolve({}));
  const removeSessionExtension = vi.fn(() => Promise.resolve({}));
  const listLibrary = vi.fn(() => Promise.resolve({ items: [] }));
  const addLibraryText = vi.fn(() => Promise.reject(new Error('not used')));
  const addLibraryImage = vi.fn(() => Promise.reject(new Error('not used')));
  const linkLibraryFile = vi.fn(() => Promise.reject(new Error('not used')));
  const removeLibraryItem = vi.fn(() => Promise.resolve({ removed: false }));
  const resolveLibrary = vi.fn<() => Promise<ShellLibraryResolveResponse_unstable>>(() =>
    Promise.resolve({ items: [] })
  );
  const prepare = vi.fn(() => Promise.reject(new Error('not used')));
  const snapshot = vi.fn<() => Promise<DomainSnapshotResponse_unstable>>(() =>
    Promise.reject(new Error('not used'))
  );
  const action = vi.fn<() => Promise<DomainActionResponse_unstable>>(() =>
    Promise.reject(new Error('not used'))
  );
  const confirmAction = vi.fn<() => Promise<DomainActionConfirmResponse_unstable>>(() =>
    Promise.reject(new Error('not used'))
  );
  const client = {
    signal: new AbortController().signal,
    closed: new Promise<void>(() => {}),
    initialize,
    newSession,
    loadSession,
    listSessions,
    prompt,
    cancel,
    gosling: {
      sessionInfo_unstable: sessionInfo,
      shellProvisioningRead_unstable: read,
      shellProvisioningValidate_unstable: validate,
      shellDirectoryValidate_unstable: validateDirectory,
      shellCredentialsList_unstable: listCredentials,
      shellModulesList_unstable: listModules,
      extensionsAvailable_unstable: listAvailableExtensions,
      sessionExtensionsList_unstable: listSessionExtensions,
      sessionExtensionsAdd_unstable: addSessionExtension,
      sessionExtensionsRemove_unstable: removeSessionExtension,
      shellSessionArtifactsList_unstable: listArtifacts,
      shellSessionLibraryList_unstable: listLibrary,
      shellSessionLibraryAddText_unstable: addLibraryText,
      shellSessionLibraryAddImage_unstable: addLibraryImage,
      shellSessionLibraryLinkFile_unstable: linkLibraryFile,
      shellSessionLibraryRemove_unstable: removeLibraryItem,
      shellSessionLibraryResolve_unstable: resolveLibrary,
      shellHandoffPrepare_unstable: prepare,
      shellDomainSnapshot_unstable: snapshot,
      shellDomainAction_unstable: action,
      shellDomainActionConfirm_unstable: confirmAction,
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
  return {
    client,
    close,
    createClient,
    createStream,
    dependencies,
    initialize,
    loadSession,
    listSessions,
    newSession,
    prompt,
    cancel,
    read,
    sessionInfo,
    snapshot,
    action,
    confirmAction,
    resolveLibrary,
    validate,
  };
}

function connect(value = harness()) {
  return {
    value,
    promise: connectShellAcp({
      acpUrl: 'ws://127.0.0.1:7777/acp',
      acpSubprotocol: 'gosling.token.test-secret',
      profile,
      manifest,
      clientName: 'fixture-shell',
      clientVersion: '0.0.0-test',
      dependencies: value.dependencies,
    }),
  };
}

describe('shell ACP runtime', () => {
  it('projects only bounded schema paths from provisioning issues', () => {
    expect(
      provisioningIssueSummaries([
        { code: 'credential_profile_unavailable', path: 'session.credentialProfileId' },
        { code: 'invalid_path', path: '/Users/eric/.config/gosling.json' },
        { code: 'also_invalid', path: 'credential profile' },
        { code: 'no_path' },
      ])
    ).toEqual([
      { code: 'credential_profile_unavailable', path: 'session.credentialProfileId' },
      { code: 'invalid_path', path: null },
      { code: 'also_invalid', path: null },
      { code: 'no_path', path: null },
    ]);
  });

  it('initializes, reads, validates, and checks compatibility without creating a session', async () => {
    const { promise, value } = connect();
    const connection = await promise;
    expect(value.createStream).toHaveBeenCalledWith(
      'ws://127.0.0.1:7777/acp',
      'gosling.token.test-secret'
    );
    expect(value.initialize).toHaveBeenCalledWith({
      protocolVersion: 1,
      clientCapabilities: {
        elicitation: { form: {} },
        _meta: { gosling: { customNotifications: true } },
      },
      clientInfo: { name: 'fixture-shell', version: '0.0.0-test' },
    });
    expect(value.read).toHaveBeenCalledWith({});
    expect(value.validate).toHaveBeenCalledWith({});
    expect(value.newSession).not.toHaveBeenCalled();
    expect(value.loadSession).not.toHaveBeenCalled();
    const callbacks = value.createClient.mock.calls[0][0]();
    expect(Object.keys(callbacks).sort()).toEqual([
      'requestPermission',
      'sessionUpdate',
      'unstable_createElicitation',
      'unstable_shellDomainStatus',
    ]);
    expect(await callbacks.requestPermission({} as never)).toEqual({
      outcome: { outcome: 'cancelled' },
    });
    expect(await callbacks.unstable_createElicitation!({} as never)).toEqual({ action: 'decline' });
    expect(connection.compatibility).toEqual({ compatible: true });
    await expect(connection.listArtifacts('session-1')).resolves.toEqual({
      artifacts: [],
      totalCount: 0,
      truncated: false,
    });
    expect(value.newSession).not.toHaveBeenCalled();
    expect(value.loadSession).not.toHaveBeenCalled();
    expect(value.close).not.toHaveBeenCalled();
    connection.close();
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('bounds every startup preflight request and reports the phase that stalled', async () => {
    vi.useFakeTimers();
    try {
      const value = harness();
      value.read.mockImplementation(() => new Promise(() => {}));
      const phases: string[] = [];
      const connection = connectShellAcp({
        acpUrl: 'ws://127.0.0.1:7777/acp',
        acpSubprotocol: 'gosling.token.test-secret',
        profile,
        manifest,
        clientName: 'fixture-shell',
        clientVersion: '0.0.0-test',
        onPreflightPhase: (phase) => phases.push(phase),
        dependencies: value.dependencies,
      });
      const rejection = expect(connection).rejects.toThrow('ACP provisioning_read timed out');

      await vi.advanceTimersByTimeAsync(10_000);

      await rejection;
      expect(phases).toEqual(['initialize', 'methods', 'directory', 'provisioning_read']);
      expect(value.validate).not.toHaveBeenCalled();
      expect(value.close).toHaveBeenCalledOnce();
    } finally {
      vi.useRealTimers();
    }
  });

  it('reads and requires the consumer-declared live domain adapter descriptor', async () => {
    const value = harness();
    const initialized = response();
    const shell = initialized.agentCapabilities!._meta!.goslingShell as {
      availableMethods: string[];
      domainAdapter?: unknown;
    };
    shell.availableMethods.push('_gosling/unstable/shell/domain/action');
    shell.domainAdapter = {
      domainId: 'neutral-fixture',
      displayName: 'Neutral Fixture',
      version: '0.1.0',
      protocolVersion: '1.0.0',
      actions: [
        { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
        { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
      ],
    };
    value.initialize.mockResolvedValue(initialized);
    const adapterManifest = { ...manifest };
    adapterManifest.consumer = {
      consumerId: 'fixture-consumer',
      consumerHash: 'c'.repeat(64),
      rendererHash: 'd'.repeat(64),
      declaredCapabilities: ['domain.action'],
      requiredAgentCapabilities: [],
      requiredMethods: [...methods, '_gosling/unstable/shell/domain/action'],
      domainAdapter: {
        descriptorId: 'neutral-fixture',
        protocolVersion: '1.0.0',
        actions: ['inspect', 'toggle'],
      },
    };

    const connection = await connectShellAcp({
      acpUrl: 'ws://127.0.0.1:7777/acp',
      acpSubprotocol: 'gosling.token.test-secret',
      profile,
      manifest: adapterManifest,
      clientName: 'fixture-shell',
      clientVersion: '0.0.0-test',
      dependencies: value.dependencies,
    });

    expect(connection.domainAdapter).toEqual({
      descriptorId: 'neutral-fixture',
      protocolVersion: '1.0.0',
      actions: ['inspect', 'toggle'],
    });
  });

  it('rejects a consumer-declared custom method that the live server did not advertise', async () => {
    const value = harness();
    const consumerManifest = { ...manifest };
    consumerManifest.consumer = {
      consumerId: 'fixture-consumer',
      consumerHash: 'c'.repeat(64),
      rendererHash: 'd'.repeat(64),
      declaredCapabilities: ['confirmation.respond'],
      requiredAgentCapabilities: [],
      requiredMethods: [...methods, '_gosling/unstable/shell/domain/action/confirm'],
    };

    await expect(
      connectShellAcp({
        acpUrl: 'ws://127.0.0.1:7777/acp',
        acpSubprotocol: 'gosling.token.test-secret',
        profile,
        manifest: consumerManifest,
        clientName: 'fixture-shell',
        clientVersion: '0.0.0-test',
        dependencies: value.dependencies,
      })
    ).rejects.toMatchObject({ name: 'ShellCompatibilityError', message: 'METHOD_UNAVAILABLE' });
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('creates and resumes sessions only through the compatible main-owned connection', async () => {
    const { promise, value } = connect();
    const connection = await promise;

    await expect(connection.createSession({ workingDir: '/workspace/current' })).resolves.toEqual({
      sessionId: 'session-1',
      workingDir: '/workspace/current',
      title: null,
      providerId: null,
      modelId: null,
    });
    expect(value.newSession).toHaveBeenCalledWith({
      cwd: '/workspace/current',
      mcpServers: [],
      _meta: { client: 'gosling-shell' },
    });

    await expect(connection.resumeSession('session-1', '/workspace/saved')).resolves.toEqual({
      sessionId: 'session-1',
      workingDir: '/workspace/saved',
      title: null,
      providerId: null,
      modelId: null,
      resumeIntegrity: 'clean',
    });
    expect(value.sessionInfo).toHaveBeenCalledWith({ sessionId: 'session-1' });
    expect(value.loadSession).toHaveBeenCalledWith({
      sessionId: 'session-1',
      cwd: '/workspace/saved',
      mcpServers: [],
      _meta: { gosling: { loadMode: 'compacted', tailLimit: 50 } },
    });
  });

  it('refuses to load a session outside the main-selected working directory', async () => {
    const { promise, value } = connect();
    const connection = await promise;

    await expect(connection.resumeSession('session-1', '/workspace/current')).rejects.toThrow(
      'does not match the selected directory'
    );
    expect(value.sessionInfo).toHaveBeenCalledWith({ sessionId: 'session-1' });
    expect(value.loadSession).not.toHaveBeenCalled();
  });

  it('lists only bounded current-directory ACP session summaries without message snippets', async () => {
    const { promise, value } = connect();
    value.listSessions.mockResolvedValueOnce({
      sessions: [
        {
          sessionId: 'session-1',
          cwd: '/workspace/current',
          title: 'Current task',
          updatedAt: '2026-08-15T12:00:00Z',
          _meta: { providerId: 'provider-a', modelId: 'model-a', messageCount: 4 },
        },
        {
          sessionId: 'session-other-workspace',
          cwd: '/workspace/other',
          title: 'Must not cross the selected workspace boundary',
          updatedAt: '2026-08-14T11:00:00Z',
        },
      ],
    } as never);
    const connection = await promise;

    await expect(connection.listSessions('/workspace/current')).resolves.toEqual([
      {
        sessionId: 'session-1',
        workingDir: '/workspace/current',
        title: 'Current task',
        providerId: 'provider-a',
        modelId: 'model-a',
        updatedAt: '2026-08-15T12:00:00Z',
        messageCount: 4,
      },
    ]);
    expect(value.listSessions).toHaveBeenCalledWith({
      cwd: '/workspace/current',
      _meta: {
        types: ['acp'],
        gosling: { archiveState: 'active', includeLastMessageSnippet: false },
      },
    });
  });

  it('submits bounded text and cancels only through the main-owned ACP connection', async () => {
    const { promise, value } = connect();
    const connection = await promise;

    await connection.prompt({ sessionId: 'session-1', text: 'hello', messageId: 'attempt-1' });
    await connection.cancel({ sessionId: 'session-1' });

    expect(value.prompt).toHaveBeenCalledWith({
      sessionId: 'session-1',
      messageId: 'attempt-1',
      prompt: [{ type: 'text', text: 'hello' }],
    });
    expect(value.cancel).toHaveBeenCalledWith({ sessionId: 'session-1' });
  });

  it('resolves selected library items into standard ACP text and image blocks', async () => {
    const { promise, value } = connect();
    value.resolveLibrary.mockResolvedValue({
      items: [
        { id: 'lib-text', name: 'Notes', content: { type: 'text', text: 'library notes' } },
        {
          id: 'lib-image',
          name: 'Sketch',
          content: { type: 'image', data: 'aW1hZ2U=', mime_type: 'image/png' },
        },
      ],
    });
    const connection = await promise;

    await connection.prompt({
      sessionId: 'session-1',
      text: '',
      messageId: 'attempt-library',
      libraryItemIds: ['lib-text', 'lib-image'],
    });

    expect(value.resolveLibrary).toHaveBeenCalledWith({
      sessionId: 'session-1',
      itemIds: ['lib-text', 'lib-image'],
    });
    expect(value.prompt).toHaveBeenCalledWith({
      sessionId: 'session-1',
      messageId: 'attempt-library',
      prompt: [
        { type: 'text', text: 'library notes' },
        { type: 'image', data: 'aW1hZ2U=', mimeType: 'image/png' },
      ],
    });
  });

  it('relays domain actions and confirmation only through the main-owned connection', async () => {
    const { promise, value } = connect();
    value.snapshot.mockResolvedValue({ domainId: 'neutral-fixture', payload: {}, resources: [] });
    value.action.mockResolvedValue({
      domainId: 'neutral-fixture',
      action: 'toggle',
      confirmationActionId: 'confirm-a',
    });
    value.confirmAction.mockResolvedValue({ status: 'denied' });
    const connection = await promise;

    await connection.domainSnapshot({ input: { scope: 'neutral' } });
    await connection.domainAction({
      sessionId: 'session-1',
      generation: 1,
      action: 'toggle',
      input: { enabled: true },
    });
    await connection.confirmDomainAction({
      sessionId: 'session-1',
      generation: 1,
      actionId: 'confirm-a',
      approve: false,
    });

    expect(value.snapshot).toHaveBeenCalledWith({ input: { scope: 'neutral' } });
    expect(value.action).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      action: 'toggle',
      input: { enabled: true },
    });
    expect(value.confirmAction).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      actionId: 'confirm-a',
      approve: false,
    });
  });

  it('rejects a relative main-supplied working directory before reaching the server', async () => {
    const { promise, value } = connect();
    const connection = await promise;

    await expect(connection.createSession({ workingDir: 'relative' })).rejects.toThrow(
      'workingDir must be an absolute path'
    );
    await expect(connection.validateDirectory('relative')).rejects.toThrow(
      'workingDir must be an absolute path'
    );
    expect(value.newSession).not.toHaveBeenCalled();
  });

  it('rejects invalid resume IDs before contacting the backend', async () => {
    const { promise, value } = connect();
    const connection = await promise;

    await expect(connection.resumeSession(' padded ', '/workspace/current')).rejects.toThrow(
      'sessionId'
    );
    expect(value.sessionInfo).not.toHaveBeenCalled();
    expect(value.loadSession).not.toHaveBeenCalled();
  });

  it('rejects missing required session-info method before provisioning or session use', async () => {
    const value = harness();
    const missing = response();
    const shell = missing.agentCapabilities!._meta!.goslingShell as {
      availableMethods: string[];
    };
    shell.availableMethods = shell.availableMethods.filter(
      (method) => method !== sessionInfoMethod
    );
    value.initialize.mockResolvedValue(missing);

    await expect(connect(value).promise).rejects.toMatchObject({
      result: { compatible: false, code: 'METHOD_UNAVAILABLE' },
    });
    expect(value.read).not.toHaveBeenCalled();
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('requires canonical load-session capability before provisioning or session use', async () => {
    const value = harness();
    const missing = response();
    missing.agentCapabilities!.loadSession = false;
    value.initialize.mockResolvedValue(missing);

    await expect(connect(value).promise).rejects.toThrow('required load-session capability');
    expect(value.read).not.toHaveBeenCalled();
    expect(value.newSession).not.toHaveBeenCalled();
    expect(value.close).toHaveBeenCalledOnce();
  });

  // A credential in the URL is now itself non-canonical: the secret belongs in
  // the WebSocket subprotocol (SEC-GOS-001).
  it.each([
    'ws://localhost:7777/acp',
    'https://127.0.0.1:7777/acp',
    'ws://127.0.0.1:7777/other',
    'ws://user:pass@127.0.0.1:7777/acp',
    'ws://127.0.0.1:7777/acp?token=private',
    'ws://127.0.0.1:7777/acp?extra=value',
  ])('rejects non-canonical ACP endpoint %s before creating a transport', async (acpUrl) => {
    const value = harness();
    await expect(
      connectShellAcp({
        acpUrl,
        acpSubprotocol: 'gosling.token.test-secret',
        profile,
        manifest,
        clientName: 'fixture-shell',
        clientVersion: '0.0.0-test',
        dependencies: value.dependencies,
      })
    ).rejects.toThrow(/ACP endpoint/);
    expect(value.createStream).not.toHaveBeenCalled();
  });

  it('passes the credential to the transport as a subprotocol, not in the URL', async () => {
    const value = harness();
    void connectShellAcp({
      acpUrl: 'ws://127.0.0.1:7777/acp',
      acpSubprotocol: 'gosling.token.test-secret',
      profile,
      manifest,
      clientName: 'fixture-shell',
      clientVersion: '0.0.0-test',
      dependencies: value.dependencies,
    });
    await vi.waitFor(() => expect(value.createStream).toHaveBeenCalled());
    expect(value.createStream).toHaveBeenCalledWith(
      'ws://127.0.0.1:7777/acp',
      'gosling.token.test-secret'
    );
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
    shell.availableMethods = shell.availableMethods.filter(
      (method) => method !== '_gosling/unstable/shell/provisioning/validate'
    );
    value.initialize.mockResolvedValue(incompatible);
    await expect(connect(value).promise).rejects.toMatchObject({
      result: { compatible: false, code: 'METHOD_UNAVAILABLE' },
    });
    expect(value.read).not.toHaveBeenCalled();
    expect(value.close).toHaveBeenCalledOnce();
  });

  it('requires the methods main always uses, even when nothing declares them', async () => {
    const declaredNothing: ShellBuildManifest = {
      ...manifest,
      compatibility: { ...manifest.compatibility, requiredMethods: [sessionInfoMethod] },
    };
    const bareProfile: ResolvedShellProductProfile = {
      ...profile,
      compatibility: { ...profile.compatibility, requiredMethods: [sessionInfoMethod] },
    };

    for (const missing of mainOwnedMethods) {
      const value = harness();
      const incompatible = response();
      const shell = incompatible.agentCapabilities!._meta!.goslingShell as {
        availableMethods: string[];
      };
      shell.availableMethods = shell.availableMethods.filter((method) => method !== missing);
      value.initialize.mockResolvedValue(incompatible);

      await expect(
        connectShellAcp({
          acpUrl: 'ws://127.0.0.1:7777/acp',
          acpSubprotocol: 'gosling.token.test-secret',
          profile: bareProfile,
          manifest: declaredNothing,
          clientName: 'fixture-shell',
          clientVersion: '0.0.0-test',
          dependencies: value.dependencies,
        })
      ).rejects.toMatchObject({ result: { compatible: false, code: 'METHOD_UNAVAILABLE' } });
      expect(value.read).not.toHaveBeenCalled();
    }
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

  it('rejects a backend runtime namespace that differs from the consumer profile', async () => {
    const value = harness();
    const incompatible = response();
    const shell = incompatible.agentCapabilities!._meta!.goslingShell as {
      identity: { runtimeNamespace: string };
      availableMethods: string[];
    };
    shell.identity.runtimeNamespace = 'other-runtime';
    value.initialize.mockResolvedValue(incompatible);

    await expect(connect(value).promise).rejects.toMatchObject({
      result: { compatible: false, code: 'RUNTIME_NAMESPACE_MISMATCH' },
    });
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
          runtimeNamespace: product.runtimeNamespace,
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
