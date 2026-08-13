import {
  GoslingClient,
  type GetSessionInfoResponse_unstable,
  type GoslingClientCallbacks,
  type ShellHandoffPrepareRequest_unstable,
  type ShellHandoffPrepareResponse_unstable,
  type ShellProvisioningReadResponse_unstable,
  type ShellProvisioningValidateResponse_unstable,
} from '@repo-makeover/gosling-sdk';
import {
  PROTOCOL_VERSION,
  type InitializeResponse,
  type LoadSessionResponse,
  type NewSessionResponse,
} from '@agentclientprotocol/sdk';
import path from 'node:path';
import type { ClosableAcpStream } from '../acp/createWebSocketStream';
import { createWebSocketStream } from '../acp/createWebSocketStream';
import {
  checkShellCompatibility,
  checkShellMethods,
  type ShellCompatibilityResult,
} from './compatibility';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';

const ACP_INITIALIZE_TIMEOUT_MS = 10_000;

export interface ShellSession {
  sessionId: string;
  workingDir: string;
}

export interface ShellAcpClient {
  signal: globalThis.AbortSignal;
  closed: Promise<void>;
  initialize(params: Parameters<GoslingClient['initialize']>[0]): Promise<InitializeResponse>;
  newSession(params: Parameters<GoslingClient['newSession']>[0]): Promise<NewSessionResponse>;
  loadSession(params: Parameters<GoslingClient['loadSession']>[0]): Promise<LoadSessionResponse>;
  gosling: {
    sessionInfo_unstable(params: { sessionId: string }): Promise<GetSessionInfoResponse_unstable>;
    shellProvisioningRead_unstable(
      params: Record<string, never>
    ): Promise<ShellProvisioningReadResponse_unstable>;
    shellProvisioningValidate_unstable(params: {
      provisioning: ShellProvisioningReadResponse_unstable['provisioning'];
    }): Promise<ShellProvisioningValidateResponse_unstable>;
    shellHandoffPrepare_unstable(
      params: ShellHandoffPrepareRequest_unstable
    ): Promise<ShellHandoffPrepareResponse_unstable>;
  };
}

export interface ShellAcpConnection {
  client: ShellAcpClient;
  initializeResponse: InitializeResponse;
  provisioning: ShellProvisioningValidateResponse_unstable;
  compatibility: { compatible: true };
  createSession(): Promise<ShellSession>;
  resumeSession(sessionId: string): Promise<ShellSession>;
  prepareHandoff(
    request: ShellHandoffPrepareRequest_unstable
  ): Promise<ShellHandoffPrepareResponse_unstable>;
  closed: Promise<void>;
  close(): void;
}

export class ShellCompatibilityError extends Error {
  constructor(readonly result: Exclude<ShellCompatibilityResult, { compatible: true }>) {
    super(result.code);
    this.name = 'ShellCompatibilityError';
  }
}

export interface ShellAcpRuntimeDependencies {
  createStream(url: string): ClosableAcpStream;
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
  if (parsed.pathname !== '/acp' || !parsed.searchParams.get('token')) {
    throw new Error('ACP endpoint must carry the server authentication token');
  }
  if (
    parsed.username ||
    parsed.password ||
    [...parsed.searchParams.keys()].some((key) => key !== 'token')
  ) {
    throw new Error('ACP endpoint contains unsupported authority');
  }
  return parsed.href;
}

function clientCallbacks(): () => GoslingClientCallbacks {
  return () => ({
    requestPermission: async () => ({ outcome: { outcome: 'cancelled' } }),
    unstable_createElicitation: async () => ({ action: 'decline' }),
    sessionUpdate: async () => {},
  });
}

function withTimeout<T>(
  promise: Promise<T>,
  milliseconds: number,
  dependencies: ShellAcpRuntimeDependencies
): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<T>((_, reject) => {
    timeoutId = dependencies.setTimeout(
      () => reject(new Error('ACP initialization timed out')),
      milliseconds
    );
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timeoutId !== undefined) {
      dependencies.clearTimeout(timeoutId);
    }
  });
}

function readShellMetadata(response: InitializeResponse): {
  identity: { id: string; displayName: string; version: string };
  availableMethods: string[];
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
    typeof fixed.version !== 'string'
  ) {
    throw new Error('ACP shell metadata returned invalid identity');
  }
  if (
    !Array.isArray(metadata.availableMethods) ||
    metadata.availableMethods.some((method) => typeof method !== 'string')
  ) {
    throw new Error('ACP shell metadata omitted available methods');
  }
  return {
    identity: { id: fixed.id, displayName: fixed.displayName, version: fixed.version },
    availableMethods: [...metadata.availableMethods],
  };
}

function agentVersion(response: InitializeResponse): string {
  const version = response.agentInfo?.version;
  if (typeof version !== 'string' || version.length === 0) {
    throw new Error('ACP initialization omitted the core version');
  }
  return version;
}

function assertSessionCapabilities(response: InitializeResponse): void {
  if (response.agentCapabilities?.loadSession !== true) {
    throw new Error('ACP initialization omitted the required load-session capability');
  }
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

function asShellSession(sessionId: string, workingDir: string): ShellSession {
  return {
    sessionId: assertSessionId(sessionId),
    workingDir: assertAbsoluteWorkingDir(workingDir),
  };
}

export async function connectShellAcp(input: {
  acpUrl: string;
  profile: ResolvedShellProductProfile;
  manifest: ShellBuildManifest;
  workingDir: string;
  clientName: string;
  clientVersion: string;
  dependencies?: ShellAcpRuntimeDependencies;
}): Promise<ShellAcpConnection> {
  const dependencies = input.dependencies ?? defaultDependencies;
  const workingDir = assertAbsoluteWorkingDir(input.workingDir);
  const stream = dependencies.createStream(assertAuthenticatedLoopbackUrl(input.acpUrl));
  const client = dependencies.createClient(clientCallbacks(), stream);

  try {
    const initializeResponse = await withTimeout(
      client.initialize({
        protocolVersion: PROTOCOL_VERSION,
        clientCapabilities: {},
        clientInfo: { name: input.clientName, version: input.clientVersion },
      }),
      ACP_INITIALIZE_TIMEOUT_MS,
      dependencies
    );
    const metadata = readShellMetadata(initializeResponse);
    assertSessionCapabilities(initializeResponse);
    const methodCompatibility = checkShellMethods(
      input.profile.compatibility.requiredMethods,
      metadata.availableMethods
    );
    if (!methodCompatibility.compatible) {
      throw new ShellCompatibilityError(methodCompatibility);
    }
    const read = await client.gosling.shellProvisioningRead_unstable({});
    const validation = await client.gosling.shellProvisioningValidate_unstable({
      provisioning: read.provisioning,
    });
    const compatibility = checkShellCompatibility({
      profile: input.profile,
      manifest: input.manifest,
      runtime: {
        identity: metadata.identity,
        coreVersion: agentVersion(initializeResponse),
        availableMethods: metadata.availableMethods,
      },
      provisioning: {
        schemaVersion: validation.provisioning.schemaVersion,
        identity: validation.provisioning.identity,
        valid: read.validation.valid && validation.validation.valid,
      },
    });
    if (!compatibility.compatible) {
      throw new ShellCompatibilityError(compatibility);
    }
    return {
      client,
      initializeResponse,
      provisioning: validation,
      compatibility,
      createSession: async () => {
        const created = await client.newSession({
          cwd: workingDir,
          mcpServers: [],
          _meta: { client: 'gosling-shell' },
        });
        return asShellSession(String(created.sessionId), workingDir);
      },
      resumeSession: async (sessionId) => {
        const fixedSessionId = assertSessionId(sessionId);
        const info = await client.gosling.sessionInfo_unstable({ sessionId: fixedSessionId });
        const session = asShellSession(String(info.session.sessionId), info.session.cwd);
        if (session.sessionId !== fixedSessionId) {
          throw new Error('session info returned a different sessionId');
        }
        await client.loadSession({
          sessionId: session.sessionId,
          cwd: session.workingDir,
          mcpServers: [],
          _meta: { gosling: { loadMode: 'compacted', tailLimit: 50 } },
        });
        return session;
      },
      prepareHandoff: (request) => client.gosling.shellHandoffPrepare_unstable(request),
      closed: client.closed,
      close: () => stream.close(),
    };
  } catch (error) {
    stream.close();
    throw error;
  }
}
