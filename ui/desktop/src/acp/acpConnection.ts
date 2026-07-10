import {
  DEFAULT_GOSLING_MCP_HOST_CAPABILITIES,
  GoslingClient,
  type GoslingClientCallbacks,
} from '@repo-makeover/gosling-sdk';
import { PROTOCOL_VERSION, type InitializeResponse } from '@agentclientprotocol/sdk';
import packageJson from '../../package.json';
import {
  handleAcpGoslingSessionNotification,
  handleAcpSessionNotification,
} from './chatNotifications';
import { createWebSocketStream } from './createWebSocketStream';
import { requestAcpElicitation } from './elicitationRequests';
import { requestAcpPermission } from './permissionRequests';

type InitializedAcpClient = {
  client: GoslingClient;
  initializeResponse: InitializeResponse;
  generation: number;
};

const ACP_INITIALIZE_TIMEOUT_MS = 10_000;

let clientPromise: Promise<InitializedAcpClient> | null = null;
let resolvedClient: InitializedAcpClient | null = null;
let nextConnectionGeneration = 1;

function createClientCallbacks(): () => GoslingClientCallbacks {
  return () => ({
    requestPermission: requestAcpPermission,
    unstable_createElicitation: requestAcpElicitation,
    sessionUpdate: handleAcpSessionNotification,
    unstable_sessionUpdate: handleAcpGoslingSessionNotification,
  });
}

function monitorConnection(client: GoslingClient): void {
  const clearClient = () => {
    if (resolvedClient?.client === client) {
      resolvedClient = null;
      clientPromise = null;
    }
  };

  client.closed.then(clearClient).catch(clearClient);
}

async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string): Promise<T> {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  const timeout = new Promise<T>((_, reject) => {
    timeoutId = setTimeout(() => reject(new Error(message)), timeoutMs);
  });

  try {
    return await Promise.race([promise, timeout]);
  } finally {
    if (timeoutId !== null) {
      clearTimeout(timeoutId);
    }
  }
}

async function initializeConnection(): Promise<InitializedAcpClient> {
  const wsUrl = await window.electron.getAcpUrl();
  if (!wsUrl) {
    throw new Error('ACP URL is not available');
  }

  const stream = createWebSocketStream(wsUrl);
  const client = new GoslingClient(createClientCallbacks(), stream);

  try {
    const initializeResponse = await withTimeout(
      client.initialize({
        protocolVersion: PROTOCOL_VERSION,
        _meta: {
          'gosling/useLoginShellPath': true,
        },
        clientCapabilities: {
          elicitation: { form: {} },
          _meta: {
            gosling: {
              mcpHostCapabilities: DEFAULT_GOSLING_MCP_HOST_CAPABILITIES,
              customNotifications: true,
            },
          },
        },
        clientInfo: {
          name: packageJson.name,
          version: packageJson.version,
        },
      }),
      ACP_INITIALIZE_TIMEOUT_MS,
      `ACP initialize timed out after ${ACP_INITIALIZE_TIMEOUT_MS}ms`
    );

    const clientState = {
      client,
      initializeResponse,
      generation: nextConnectionGeneration++,
    };
    return clientState;
  } catch (error) {
    stream.close();
    throw error;
  }
}

export async function getAcpClient(): Promise<GoslingClient> {
  return (await getInitializedAcpClient()).client;
}

export function getAcpClientSync(): GoslingClient | null {
  return resolvedClient?.client ?? null;
}

export async function getAcpInitializeResponse(): Promise<InitializeResponse> {
  return (await getInitializedAcpClient()).initializeResponse;
}

export function isAcpClientReady(): boolean {
  return resolvedClient !== null;
}

export function getAcpConnectionGeneration(): number | null {
  return resolvedClient?.generation ?? null;
}

async function getInitializedAcpClient(): Promise<InitializedAcpClient> {
  if (resolvedClient) {
    return resolvedClient;
  }

  if (!clientPromise) {
    clientPromise = initializeConnection()
      .then((clientState) => {
        resolvedClient = clientState;
        monitorConnection(clientState.client);
        return clientState;
      })
      .catch((error) => {
        clientPromise = null;
        throw error;
      });
  }

  return clientPromise;
}
