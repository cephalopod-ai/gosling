import {
  GoslingClient,
  type DomainActionConfirmRequest_unstable,
  type DomainActionConfirmResponse_unstable,
  type DomainActionRequest_unstable,
  type DomainActionResponse_unstable,
  type DomainSnapshotRequest_unstable,
  type DomainSnapshotResponse_unstable,
  type GetAvailableExtensionsResponse_unstable,
  type GetSessionExtensionsResponse_unstable,
  type GetSessionInfoResponse_unstable,
  type GoslingClientCallbacks,
  type GoslingExtension,
  type ShellCredentialListResponse_unstable,
  type ShellArtifactListResponse_unstable,
  type ShellDirectoryValidateResponse_unstable,
  type ShellHandoffPrepareRequest_unstable,
  type ShellHandoffPrepareResponse_unstable,
  type ShellModuleListResponse_unstable,
  type ShellLibraryAddImageRequest_unstable,
  type ShellLibraryAddResponse_unstable,
  type ShellLibraryAddTextRequest_unstable,
  type ShellLibraryLinkFileRequest_unstable,
  type ShellLibraryListResponse_unstable,
  type ShellLibraryRemoveResponse_unstable,
  type ShellLibraryResolveResponse_unstable,
  type ShellProvisioningReadResponse_unstable,
  type ShellProvisioningValidateRequest_unstable,
  type ShellProvisioningValidateResponse_unstable,
} from '@repo-makeover/gosling-sdk';
import {
  PROTOCOL_VERSION,
  type InitializeResponse,
  type ListSessionsRequest,
  type ListSessionsResponse,
  type LoadSessionResponse,
  type NewSessionResponse,
  type SessionInfo,
} from '@agentclientprotocol/sdk';
import path from 'node:path';
import type { ClosableAcpStream } from '../acp/createWebSocketStream';
import { createWebSocketStream } from '../acp/createWebSocketStream';
import {
  checkShellCompatibility,
  checkShellMethods,
  type ShellCompatibilityResult,
  type ShellRuntimeMetadata,
} from './compatibility';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';

const ACP_PREFLIGHT_TIMEOUT_MS = 10_000;

export type ShellAcpPreflightPhase =
  | 'initialize'
  | 'methods'
  | 'directory'
  | 'provisioning_read'
  | 'provisioning_validate'
  | 'compatibility';

export interface ShellSession {
  sessionId: string;
  workingDir: string;
  title: string | null;
  providerId: string | null;
  modelId: string | null;
  resumeIntegrity?: 'clean' | 'uncertain';
}

export interface ShellSessionSummary extends ShellSession {
  updatedAt: string | null;
  messageCount: number | null;
}

export interface ShellAcpClient {
  signal: globalThis.AbortSignal;
  closed: Promise<void>;
  initialize(params: Parameters<GoslingClient['initialize']>[0]): Promise<InitializeResponse>;
  newSession(params: Parameters<GoslingClient['newSession']>[0]): Promise<NewSessionResponse>;
  loadSession(params: Parameters<GoslingClient['loadSession']>[0]): Promise<LoadSessionResponse>;
  listSessions(params: ListSessionsRequest): Promise<ListSessionsResponse>;
  prompt(params: Parameters<GoslingClient['prompt']>[0]): ReturnType<GoslingClient['prompt']>;
  cancel(params: Parameters<GoslingClient['cancel']>[0]): ReturnType<GoslingClient['cancel']>;
  setSessionConfigOption?(
    params: Parameters<GoslingClient['setSessionConfigOption']>[0]
  ): ReturnType<GoslingClient['setSessionConfigOption']>;
  gosling: {
    defaultsRead_unstable?(params: Record<string, never>): Promise<{
      providerId?: string | null;
      modelId?: string | null;
    }>;
    defaultsSave_unstable?(params: {
      providerId: string;
      modelId?: string | null;
    }): Promise<unknown>;
    providersList_unstable?(params: Record<string, never>): Promise<{
      entries: Array<{
        providerId: string;
        providerName: string;
        configured: boolean;
        models: Array<{ id: string; name?: string | null }>;
      }>;
    }>;
    sessionInfo_unstable(params: { sessionId: string }): Promise<GetSessionInfoResponse_unstable>;
    shellProvisioningRead_unstable(params: {
      workingDir?: string;
    }): Promise<ShellProvisioningReadResponse_unstable>;
    shellProvisioningValidate_unstable(params: {
      provisioning?: ShellProvisioningValidateRequest_unstable['provisioning'];
      workingDir?: string;
    }): Promise<ShellProvisioningValidateResponse_unstable>;
    shellDirectoryValidate_unstable(params: {
      path: string;
    }): Promise<ShellDirectoryValidateResponse_unstable>;
    shellCredentialsList_unstable(
      params: Record<string, never>
    ): Promise<ShellCredentialListResponse_unstable>;
    shellModulesList_unstable(params: {
      workingDir?: string;
    }): Promise<ShellModuleListResponse_unstable>;
    extensionsAvailable_unstable(
      params: Record<string, never>
    ): Promise<GetAvailableExtensionsResponse_unstable>;
    sessionExtensionsList_unstable(params: {
      sessionId: string;
    }): Promise<GetSessionExtensionsResponse_unstable>;
    sessionExtensionsAdd_unstable(params: {
      sessionId: string;
      extension: GoslingExtension;
    }): Promise<unknown>;
    sessionExtensionsRemove_unstable(params: { sessionId: string; name: string }): Promise<unknown>;
    shellSessionArtifactsList_unstable(params: {
      sessionId: string;
    }): Promise<ShellArtifactListResponse_unstable>;
    shellSessionLibraryList_unstable(params: {
      sessionId: string;
    }): Promise<ShellLibraryListResponse_unstable>;
    shellSessionLibraryAddText_unstable(
      params: ShellLibraryAddTextRequest_unstable
    ): Promise<ShellLibraryAddResponse_unstable>;
    shellSessionLibraryAddImage_unstable(
      params: ShellLibraryAddImageRequest_unstable
    ): Promise<ShellLibraryAddResponse_unstable>;
    shellSessionLibraryLinkFile_unstable(
      params: ShellLibraryLinkFileRequest_unstable
    ): Promise<ShellLibraryAddResponse_unstable>;
    shellSessionLibraryRemove_unstable(params: {
      sessionId: string;
      itemId: string;
    }): Promise<ShellLibraryRemoveResponse_unstable>;
    shellSessionLibraryResolve_unstable(params: {
      sessionId: string;
      itemIds: string[];
    }): Promise<ShellLibraryResolveResponse_unstable>;
    shellHandoffPrepare_unstable(
      params: ShellHandoffPrepareRequest_unstable
    ): Promise<ShellHandoffPrepareResponse_unstable>;
    shellDomainSnapshot_unstable(
      params: DomainSnapshotRequest_unstable
    ): Promise<DomainSnapshotResponse_unstable>;
    shellDomainAction_unstable(
      params: DomainActionRequest_unstable
    ): Promise<DomainActionResponse_unstable>;
    shellDomainActionConfirm_unstable(
      params: DomainActionConfirmRequest_unstable
    ): Promise<DomainActionConfirmResponse_unstable>;
  };
}

export interface ShellAcpConnection {
  client: ShellAcpClient;
  initializeResponse: InitializeResponse;
  provisioning: ShellProvisioningValidateResponse_unstable;
  compatibility: { compatible: true };
  runtimeNamespace: string;
  domainAdapter: ShellRuntimeMetadata['domainAdapter'];
  createSession(input: {
    workingDir: string;
    credentialProfileId?: string | null;
  }): Promise<ShellSession>;
  setSessionProviderModel(input: {
    sessionId: string;
    providerId: string;
    modelId: string;
  }): Promise<void>;
  resumeSession(sessionId: string, workingDir: string): Promise<ShellSession>;
  listSessions(workingDir: string): Promise<ShellSessionSummary[]>;
  validateDirectory(directory: string): Promise<ShellDirectoryValidateResponse_unstable>;
  listCredentials(): Promise<ShellCredentialListResponse_unstable>;
  listModules(workingDir: string | null): Promise<ShellModuleListResponse_unstable>;
  listAvailableExtensions(): Promise<GetAvailableExtensionsResponse_unstable>;
  listSessionExtensions(sessionId: string): Promise<GetSessionExtensionsResponse_unstable>;
  addSessionExtension(sessionId: string, extension: GoslingExtension): Promise<void>;
  removeSessionExtension(sessionId: string, name: string): Promise<void>;
  listArtifacts(sessionId: string): Promise<ShellArtifactListResponse_unstable>;
  listLibrary(sessionId: string): Promise<ShellLibraryListResponse_unstable>;
  addLibraryText(
    input: ShellLibraryAddTextRequest_unstable
  ): Promise<ShellLibraryAddResponse_unstable>;
  addLibraryImage(
    input: ShellLibraryAddImageRequest_unstable
  ): Promise<ShellLibraryAddResponse_unstable>;
  linkLibraryFile(
    input: ShellLibraryLinkFileRequest_unstable
  ): Promise<ShellLibraryAddResponse_unstable>;
  removeLibraryItem(
    sessionId: string,
    itemId: string
  ): Promise<ShellLibraryRemoveResponse_unstable>;
  prompt(input: {
    sessionId: string;
    text: string;
    messageId: string;
    libraryItemIds?: string[];
  }): Promise<unknown>;
  cancel(input: { sessionId: string }): Promise<void>;
  prepareHandoff(
    request: ShellHandoffPrepareRequest_unstable
  ): Promise<ShellHandoffPrepareResponse_unstable>;
  domainSnapshot(input: DomainSnapshotRequest_unstable): Promise<DomainSnapshotResponse_unstable>;
  domainAction(input: DomainActionRequest_unstable): Promise<DomainActionResponse_unstable>;
  confirmDomainAction(
    input: DomainActionConfirmRequest_unstable
  ): Promise<DomainActionConfirmResponse_unstable>;
  closed: Promise<void>;
  close(): void;
}

export interface ShellProvisioningIssueSummary {
  code: string;
  path: string | null;
}

export class ShellCompatibilityError extends Error {
  constructor(
    readonly result: Exclude<ShellCompatibilityResult, { compatible: true }>,
    readonly provisioningIssues: ShellProvisioningIssueSummary[] = []
  ) {
    super(result.code);
    this.name = 'ShellCompatibilityError';
  }
}

export interface ShellAcpRuntimeDependencies {
  createStream(url: string, subprotocol?: string): ClosableAcpStream;
  createClient(callbacks: () => GoslingClientCallbacks, stream: ClosableAcpStream): ShellAcpClient;
  setTimeout(callback: () => void, milliseconds: number): ReturnType<typeof setTimeout>;
  clearTimeout(id: ReturnType<typeof setTimeout>): void;
}

const defaultDependencies: ShellAcpRuntimeDependencies = {
  createStream: createWebSocketStream,
  createClient: (callbacks, stream) => new GoslingClient(callbacks, stream),
  setTimeout,
  clearTimeout,
};

function assertAuthenticatedLoopbackUrl(value: string): string {
  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error('ACP endpoint must be an absolute URL');
  }
  if (!['ws:', 'wss:'].includes(parsed.protocol)) {
    throw new Error('ACP endpoint must use WebSocket transport');
  }
  if (parsed.hostname !== '127.0.0.1' && parsed.hostname !== '[::1]') {
    throw new Error('ACP endpoint must use a loopback address');
  }
  if (parsed.pathname !== '/acp') {
    throw new Error('ACP endpoint must address the /acp path');
  }
  // The secret now travels in the WebSocket subprotocol, so the URL must carry
  // no credential at all (SEC-GOS-001).
  if (parsed.username || parsed.password || [...parsed.searchParams.keys()].length > 0) {
    throw new Error('ACP endpoint contains unsupported authority');
  }
  return parsed.href;
}

function clientCallbacks(): () => GoslingClientCallbacks {
  return () => ({
    requestPermission: async () => ({ outcome: { outcome: 'cancelled' } }),
    unstable_createElicitation: async () => ({ action: 'decline' }),
    sessionUpdate: async () => {},
    unstable_shellDomainStatus: async () => {},
  });
}

function withTimeout<T>(
  promise: Promise<T>,
  milliseconds: number,
  message: string,
  dependencies: ShellAcpRuntimeDependencies
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<T>((_, reject) => {
    timeoutId = dependencies.setTimeout(() => reject(new Error(message)), milliseconds);
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timeoutId !== undefined) {
      dependencies.clearTimeout(timeoutId);
    }
  });
}

function readShellMetadata(response: InitializeResponse): {
  identity: { id: string; displayName: string; version: string };
  runtimeNamespace: string;
  availableMethods: string[];
  domainAdapter: ShellRuntimeMetadata['domainAdapter'];
} {
  const value = response.agentCapabilities?._meta?.goslingShell;
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('ACP initialization omitted shell capability metadata');
  }
  const metadata = value as Record<string, unknown>;
  const identity = metadata.identity;
  if (!identity || typeof identity !== 'object' || Array.isArray(identity)) {
    throw new Error('ACP shell metadata omitted identity');
  }
  const fixed = identity as Record<string, unknown>;
  if (
    typeof fixed.id !== 'string' ||
    typeof fixed.displayName !== 'string' ||
    typeof fixed.version !== 'string' ||
    typeof fixed.runtimeNamespace !== 'string'
  ) {
    throw new Error('ACP shell metadata returned invalid identity');
  }
  if (
    !Array.isArray(metadata.availableMethods) ||
    metadata.availableMethods.some((method) => typeof method !== 'string')
  ) {
    throw new Error('ACP shell metadata omitted available methods');
  }
  const domainAdapter = metadata.domainAdapter;
  let parsedDomainAdapter: ShellRuntimeMetadata['domainAdapter'] = null;
  if (domainAdapter !== null && domainAdapter !== undefined) {
    if (typeof domainAdapter !== 'object' || Array.isArray(domainAdapter)) {
      throw new Error('ACP shell metadata returned invalid domain adapter');
    }
    const adapter = domainAdapter as Record<string, unknown>;
    if (
      typeof adapter.domainId !== 'string' ||
      typeof adapter.protocolVersion !== 'string' ||
      !Array.isArray(adapter.actions)
    ) {
      throw new Error('ACP shell metadata returned invalid domain adapter');
    }
    const actions = adapter.actions.map((action) => {
      if (typeof action !== 'object' || action === null || Array.isArray(action)) {
        throw new Error('ACP shell metadata returned invalid domain adapter action');
      }
      const name = (action as Record<string, unknown>).name;
      if (typeof name !== 'string') {
        throw new Error('ACP shell metadata returned invalid domain adapter action');
      }
      return name;
    });
    parsedDomainAdapter = {
      descriptorId: adapter.domainId,
      protocolVersion: adapter.protocolVersion,
      actions: actions.sort(),
    };
  }
  return {
    identity: { id: fixed.id, displayName: fixed.displayName, version: fixed.version },
    runtimeNamespace: fixed.runtimeNamespace,
    availableMethods: [...metadata.availableMethods],
    domainAdapter: parsedDomainAdapter,
  };
}

function agentVersion(response: InitializeResponse): string {
  const version = response.agentInfo?.version;
  if (typeof version !== 'string' || version.length === 0) {
    throw new Error('ACP initialization omitted the core version');
  }
  return version;
}

function resumeIntegrity(info: GetSessionInfoResponse_unstable): 'clean' | 'uncertain' {
  const meta = info.session._meta;
  if (!meta || typeof meta !== 'object' || Array.isArray(meta)) return 'uncertain';
  const gosling = (meta as Record<string, unknown>).gosling;
  if (!gosling || typeof gosling !== 'object' || Array.isArray(gosling)) return 'uncertain';
  return (gosling as Record<string, unknown>).resumeIntegrity === 'clean' ? 'clean' : 'uncertain';
}

function readRuntimeNamespace(identity: unknown): string {
  if (!identity || typeof identity !== 'object' || Array.isArray(identity)) {
    throw new Error('ACP shell provisioning returned invalid identity');
  }
  const runtimeNamespace = (identity as Record<string, unknown>).runtimeNamespace;
  if (typeof runtimeNamespace !== 'string' || runtimeNamespace.length === 0) {
    throw new Error('ACP shell provisioning omitted runtime namespace');
  }
  return runtimeNamespace;
}

const SAFE_PROVISIONING_PATH = /^[A-Za-z][A-Za-z0-9.]{0,255}$/;

export function provisioningIssueSummaries(value: unknown): ShellProvisioningIssueSummary[] {
  if (!Array.isArray(value)) return [];
  return value.slice(0, 128).flatMap((issue) => {
    if (!issue || typeof issue !== 'object' || Array.isArray(issue)) return [];
    const record = issue as Record<string, unknown>;
    if (typeof record.code !== 'string' || record.code.length === 0 || record.code.length > 256) {
      return [];
    }
    return [
      {
        code: record.code,
        path:
          typeof record.path === 'string' && SAFE_PROVISIONING_PATH.test(record.path)
            ? record.path
            : null,
      },
    ];
  });
}

function assertSessionCapabilities(
  response: InitializeResponse,
  required: readonly string[]
): void {
  for (const capability of required) {
    if (capability === 'loadSession' && response.agentCapabilities?.loadSession !== true) {
      throw new Error('ACP initialization omitted the required load-session capability');
    }
    if (capability === 'sessionList' && !response.agentCapabilities?.sessionCapabilities?.list) {
      throw new Error('ACP initialization omitted the required session-list capability');
    }
  }
}

/// Methods main uses on every startup, whatever the consumer declares.
///
/// The directory is restored and the credential catalog and module inventory are read before the
/// shell is ready, so a backend missing them is incompatible rather than merely failing later:
/// without this the call would surface as a generic startup failure instead of METHOD_UNAVAILABLE.
const MAIN_OWNED_METHODS = [
  '_gosling/unstable/shell/credentials/list',
  '_gosling/unstable/shell/directory/validate',
  '_gosling/unstable/shell/modules/list',
];

function requiredMethods(
  manifest: ShellBuildManifest,
  profile: ResolvedShellProductProfile
): string[] {
  return [
    ...new Set([
      ...MAIN_OWNED_METHODS,
      ...profile.compatibility.requiredMethods,
      ...(manifest.consumer?.requiredMethods ?? []),
    ]),
  ].sort();
}

function assertSessionId(sessionId: string): string {
  if (
    typeof sessionId !== 'string' ||
    sessionId.length === 0 ||
    sessionId.length > 512 ||
    sessionId.trim() !== sessionId
  ) {
    throw new Error('sessionId must be a non-empty bounded string');
  }
  return sessionId;
}

function assertAbsoluteWorkingDir(workingDir: string): string {
  if (typeof workingDir !== 'string' || !path.isAbsolute(workingDir)) {
    throw new Error('workingDir must be an absolute path');
  }
  return path.normalize(workingDir);
}

function boundedNullable(value: unknown, maximum: number): string | null {
  return typeof value === 'string' && value.length > 0 && value.length <= maximum ? value : null;
}

function sessionMetadata(info: SessionInfo): { providerId: string | null; modelId: string | null } {
  const meta = info._meta;
  if (!meta || typeof meta !== 'object' || Array.isArray(meta)) {
    return { providerId: null, modelId: null };
  }
  return {
    providerId: boundedNullable(meta.providerId, 256),
    modelId: boundedNullable(meta.modelId, 512),
  };
}

function asShellSession(
  sessionId: string,
  workingDir: string,
  details?: { title?: unknown; providerId?: unknown; modelId?: unknown }
): ShellSession {
  return {
    sessionId: assertSessionId(sessionId),
    workingDir: assertAbsoluteWorkingDir(workingDir),
    title: boundedNullable(details?.title, 4 * 1024),
    providerId: boundedNullable(details?.providerId, 256),
    modelId: boundedNullable(details?.modelId, 512),
  };
}

function asShellSessionSummary(info: SessionInfo): ShellSessionSummary {
  const metadata = sessionMetadata(info);
  const messageCount = info._meta?.messageCount;
  return {
    ...asShellSession(String(info.sessionId), info.cwd, {
      title: boundedNullable(info.title, 512),
      ...metadata,
    }),
    updatedAt: boundedNullable(info.updatedAt, 128),
    messageCount:
      typeof messageCount === 'number' && Number.isSafeInteger(messageCount) && messageCount >= 0
        ? messageCount
        : null,
  };
}

export async function connectShellAcp(input: {
  acpUrl: string;
  acpSubprotocol: string;
  profile: ResolvedShellProductProfile;
  manifest: ShellBuildManifest;
  clientName: string;
  clientVersion: string;
  callbacks?: () => GoslingClientCallbacks;
  /// Resolves the working directory provisioning must be judged against.
  ///
  /// It runs after the method check and before provisioning is validated, because extensions and
  /// skills can be project-local: judging a shell against the backend's startup directory would
  /// fail a product whose selected directory is the one that makes its provisioning valid.
  resolveWorkingDir?: (
    validate: (directory: string) => Promise<ShellDirectoryValidateResponse_unstable>
  ) => Promise<string | null>;
  onPreflightPhase?: (phase: ShellAcpPreflightPhase) => void;
  dependencies?: ShellAcpRuntimeDependencies;
}): Promise<ShellAcpConnection> {
  const dependencies = input.dependencies ?? defaultDependencies;
  const stream = dependencies.createStream(
    assertAuthenticatedLoopbackUrl(input.acpUrl),
    input.acpSubprotocol
  );
  const client = dependencies.createClient(input.callbacks ?? clientCallbacks(), stream);
  const preflight = <T>(promise: Promise<T>, phase: ShellAcpPreflightPhase) => {
    input.onPreflightPhase?.(phase);
    return withTimeout(promise, ACP_PREFLIGHT_TIMEOUT_MS, `ACP ${phase} timed out`, dependencies);
  };

  try {
    const initializeResponse = await preflight(
      client.initialize({
        protocolVersion: PROTOCOL_VERSION,
        clientCapabilities: {
          elicitation: { form: {} },
          _meta: { gosling: { customNotifications: true } },
        },
        clientInfo: { name: input.clientName, version: input.clientVersion },
      }),
      'initialize'
    );
    const metadata = readShellMetadata(initializeResponse);
    assertSessionCapabilities(
      initializeResponse,
      input.manifest.consumer?.requiredAgentCapabilities ?? ['loadSession']
    );
    input.onPreflightPhase?.('methods');
    const methodCompatibility = checkShellMethods(
      requiredMethods(input.manifest, input.profile),
      metadata.availableMethods
    );
    if (!methodCompatibility.compatible) {
      throw new ShellCompatibilityError(methodCompatibility);
    }
    const validateDirectory = async (directory: string) =>
      client.gosling.shellDirectoryValidate_unstable({
        path: assertAbsoluteWorkingDir(directory),
      });
    const workingDir =
      (await preflight(
        input.resolveWorkingDir?.(validateDirectory) ?? Promise.resolve(null),
        'directory'
      )) ?? null;
    const provisioningScope = workingDir === null ? {} : { workingDir };
    const read = await preflight(
      client.gosling.shellProvisioningRead_unstable(provisioningScope),
      'provisioning_read'
    );
    const validation = await preflight(
      client.gosling.shellProvisioningValidate_unstable(provisioningScope),
      'provisioning_validate'
    );
    input.onPreflightPhase?.('compatibility');
    const compatibility = checkShellCompatibility({
      profile: input.profile,
      manifest: input.manifest,
      runtime: {
        identity: metadata.identity,
        runtimeNamespace: metadata.runtimeNamespace,
        coreVersion: agentVersion(initializeResponse),
        availableMethods: metadata.availableMethods,
        domainAdapter: metadata.domainAdapter,
      },
      provisioning: {
        schemaVersion: validation.provisioning.schemaVersion,
        identity: validation.provisioning.identity,
        runtimeNamespace: readRuntimeNamespace(validation.provisioning.identity),
        valid: read.validation.valid && validation.validation.valid,
      },
    });
    if (!compatibility.compatible) {
      throw new ShellCompatibilityError(
        compatibility,
        provisioningIssueSummaries(validation.validation.issues)
      );
    }
    return {
      client,
      initializeResponse,
      provisioning: validation,
      compatibility,
      runtimeNamespace: metadata.runtimeNamespace,
      domainAdapter: metadata.domainAdapter,
      createSession: async ({ workingDir: requested, credentialProfileId }) => {
        const cwd = assertAbsoluteWorkingDir(requested);
        const created = await client.newSession({
          cwd,
          mcpServers: [],
          _meta: {
            client: 'gosling-shell',
            ...(credentialProfileId ? { shellCredentialProfileId: credentialProfileId } : {}),
          },
        });
        return asShellSession(String(created.sessionId), cwd, {
          modelId: created.models?.currentModelId,
        });
      },
      setSessionProviderModel: async ({ sessionId, providerId, modelId }) => {
        const fixedSessionId = assertSessionId(sessionId);
        if (!client.setSessionConfigOption) {
          throw new Error('model selection is unavailable');
        }
        await client.setSessionConfigOption({
          sessionId: fixedSessionId,
          configId: 'provider',
          value: providerId,
        });
        await client.setSessionConfigOption({
          sessionId: fixedSessionId,
          configId: 'model',
          value: modelId,
        });
      },
      resumeSession: async (sessionId, workingDir) => {
        const fixedSessionId = assertSessionId(sessionId);
        const expectedWorkingDir = assertAbsoluteWorkingDir(workingDir);
        const info = await client.gosling.sessionInfo_unstable({ sessionId: fixedSessionId });
        const metadata = sessionMetadata(info.session);
        const session = {
          ...asShellSession(String(info.session.sessionId), info.session.cwd, {
            title: info.session.title,
            ...metadata,
          }),
          resumeIntegrity: resumeIntegrity(info),
        };
        if (session.sessionId !== fixedSessionId) {
          throw new Error('session info returned a different sessionId');
        }
        if (session.workingDir !== expectedWorkingDir) {
          throw new Error('session working directory does not match the selected directory');
        }
        await client.loadSession({
          sessionId: session.sessionId,
          cwd: session.workingDir,
          mcpServers: [],
          _meta: { gosling: { loadMode: 'compacted', tailLimit: 50 } },
        });
        return session;
      },
      listSessions: async (workingDir) => {
        const cwd = assertAbsoluteWorkingDir(workingDir);
        const response = await withTimeout(
          client.listSessions({
            cwd,
            _meta: {
              types: ['acp'],
              gosling: { archiveState: 'active', includeLastMessageSnippet: false },
            },
          }),
          ACP_PREFLIGHT_TIMEOUT_MS,
          'ACP session list timed out',
          dependencies
        );
        return response.sessions
          .map(asShellSessionSummary)
          .filter((session) => session.workingDir === cwd)
          .slice(0, 20);
      },
      prompt: async ({ sessionId, text, messageId, libraryItemIds = [] }) => {
        const fixedSessionId = assertSessionId(sessionId);
        const resolved =
          libraryItemIds.length === 0
            ? { items: [] }
            : await client.gosling.shellSessionLibraryResolve_unstable({
                sessionId: fixedSessionId,
                itemIds: libraryItemIds,
              });
        const prompt: Parameters<GoslingClient['prompt']>[0]['prompt'] = [];
        if (text.trim().length > 0) prompt.push({ type: 'text', text });
        for (const item of resolved.items ?? []) {
          if (item.content.type === 'text') {
            prompt.push({ type: 'text', text: item.content.text });
          } else {
            prompt.push({
              type: 'image',
              data: item.content.data,
              mimeType: item.content.mime_type,
            });
          }
        }
        return client.prompt({ sessionId: fixedSessionId, messageId, prompt });
      },
      cancel: ({ sessionId }) => client.cancel({ sessionId: assertSessionId(sessionId) }),
      validateDirectory: (directory) =>
        withTimeout(
          validateDirectory(directory),
          ACP_PREFLIGHT_TIMEOUT_MS,
          'ACP directory validation timed out',
          dependencies
        ),
      listCredentials: () =>
        withTimeout(
          client.gosling.shellCredentialsList_unstable({}),
          ACP_PREFLIGHT_TIMEOUT_MS,
          'ACP credential catalog timed out',
          dependencies
        ),
      listModules: async (workingDir) =>
        withTimeout(
          client.gosling.shellModulesList_unstable(
            workingDir === null ? {} : { workingDir: assertAbsoluteWorkingDir(workingDir) }
          ),
          ACP_PREFLIGHT_TIMEOUT_MS,
          'ACP module inventory timed out',
          dependencies
        ),
      listAvailableExtensions: () =>
        withTimeout(
          client.gosling.extensionsAvailable_unstable({}),
          ACP_PREFLIGHT_TIMEOUT_MS,
          'ACP available extension inventory timed out',
          dependencies
        ),
      listSessionExtensions: (sessionId) =>
        withTimeout(
          client.gosling.sessionExtensionsList_unstable({ sessionId: assertSessionId(sessionId) }),
          ACP_PREFLIGHT_TIMEOUT_MS,
          'ACP session extension inventory timed out',
          dependencies
        ),
      addSessionExtension: async (sessionId, extension) => {
        await client.gosling.sessionExtensionsAdd_unstable({
          sessionId: assertSessionId(sessionId),
          extension,
        });
      },
      removeSessionExtension: async (sessionId, name) => {
        await client.gosling.sessionExtensionsRemove_unstable({
          sessionId: assertSessionId(sessionId),
          name,
        });
      },
      listArtifacts: (sessionId) =>
        withTimeout(
          client.gosling.shellSessionArtifactsList_unstable({
            sessionId: assertSessionId(sessionId),
          }),
          ACP_PREFLIGHT_TIMEOUT_MS,
          'ACP artifact inventory timed out',
          dependencies
        ),
      listLibrary: (sessionId) =>
        client.gosling.shellSessionLibraryList_unstable({
          sessionId: assertSessionId(sessionId),
        }),
      addLibraryText: (request) =>
        client.gosling.shellSessionLibraryAddText_unstable({
          ...request,
          sessionId: assertSessionId(request.sessionId),
        }),
      addLibraryImage: (request) =>
        client.gosling.shellSessionLibraryAddImage_unstable({
          ...request,
          sessionId: assertSessionId(request.sessionId),
        }),
      linkLibraryFile: (request) =>
        client.gosling.shellSessionLibraryLinkFile_unstable({
          ...request,
          sessionId: assertSessionId(request.sessionId),
        }),
      removeLibraryItem: (sessionId, itemId) =>
        client.gosling.shellSessionLibraryRemove_unstable({
          sessionId: assertSessionId(sessionId),
          itemId,
        }),
      prepareHandoff: (request) => client.gosling.shellHandoffPrepare_unstable(request),
      domainSnapshot: (request) => client.gosling.shellDomainSnapshot_unstable(request),
      domainAction: (request) => client.gosling.shellDomainAction_unstable(request),
      confirmDomainAction: (request) => client.gosling.shellDomainActionConfirm_unstable(request),
      closed: client.closed,
      close: () => stream.close(),
    };
  } catch (error) {
    stream.close();
    throw error;
  }
}
