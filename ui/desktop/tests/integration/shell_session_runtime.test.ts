import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { createServer } from 'node:http';
import type { Socket } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { afterEach, describe, expect, it } from 'vitest';
import {
  createShellRuntimeController,
  type ShellRuntimeController,
} from '../../src/shell/runtimeController';
import type { ResolvedShellProductProfile, ShellBuildManifest } from '../../src/shell/profile';

const repositoryRoot = path.resolve(import.meta.dirname, '../../../..');
const binaryPath = path.join(repositoryRoot, 'target', 'debug', 'gosling');
const namespace = 'shell-session-integration';
const methods = [
  '_gosling/unstable/session/info',
  '_gosling/unstable/shell/handoff/prepare',
  '_gosling/unstable/shell/provisioning/read',
  '_gosling/unstable/shell/provisioning/validate',
];
const domainMethods = [
  '_gosling/unstable/shell/domain/action',
  '_gosling/unstable/shell/domain/action/confirm',
  '_gosling/unstable/shell/domain/snapshot',
];
const product = {
  id: 'gosling-shell-session-fixture',
  displayName: 'Gosling Shell Session Fixture',
  version: '0.0.0-test',
  runtimeNamespace: namespace,
  protocolScheme: 'gosling-shell-session-fixture',
  executableName: 'gosling-shell-session-fixture',
  macosBundleId: 'io.github.repo-makeover.gosling.shell-session-fixture',
  windowsAppId: 'Gosling.Shell.Session.Fixture',
  linuxPackageName: 'gosling-shell-session-fixture',
  flatpakId: 'io.github.repo_makeover.Gosling.ShellSessionFixture',
};
const profile: ResolvedShellProductProfile = {
  schemaVersion: 1,
  product,
  provisioningPath: 'provisioning.json',
  compatibility: {
    goslingVersion: '0.1.0',
    goslingRevision: 'current',
    provisioningSchemaVersion: 1,
    handoffSchemaVersion: 1,
    requiredMethods: methods,
  },
  assets: {
    root: 'assets',
    iconBase: 'assets/icon',
    requiredTargets: ['linux-x64'],
  },
  update: { enabled: false, channel: 'fixture-disabled' },
  distribution: {
    publishable: false,
    artifactPrefix: 'gosling-shell-session-fixture',
    signingPolicy: 'none',
  },
};

const roots: string[] = [];
const controllers: ShellRuntimeController[] = [];

function manifest(version = '0.1.0'): ShellBuildManifest {
  return {
    schemaVersion: 1,
    profileSchemaVersion: 1,
    profileHash: 'a'.repeat(64),
    product,
    target: 'linux-x64',
    platform: 'linux',
    architecture: 'x64',
    sourceClean: false,
    compatibility: {
      goslingVersion: version,
      goslingRevision: 'b'.repeat(40),
      provisioningSchemaVersion: 1,
      handoffSchemaVersion: 1,
      requiredMethods: methods,
    },
  };
}

function adapterManifest(): ShellBuildManifest {
  const resolved = manifest();
  resolved.consumer = {
    consumerId: 'neutral-adapter-consumer',
    consumerHash: 'c'.repeat(64),
    rendererHash: 'd'.repeat(64),
    declaredCapabilities: ['confirmation.respond', 'domain.action', 'domain.snapshot'],
    requiredAgentCapabilities: [],
    requiredMethods: [...methods, ...domainMethods].sort(),
    domainAdapter: {
      descriptorId: 'neutral-fixture',
      protocolVersion: '1.0.0',
      actions: ['inspect', 'toggle'],
    },
  };
  return resolved;
}

function writeNeutralAdapter(root: string, versionMismatch = false, hangAction = false): string {
  const adapterPath = path.join(root, 'neutral-adapter.mjs');
  const pidPath = JSON.stringify(path.join(root, 'neutral-adapter.pid'));
  const actionStartedPath = JSON.stringify(path.join(root, 'neutral-adapter-action.started'));
  const termPath = JSON.stringify(path.join(root, 'neutral-adapter-sigterm.received'));
  const adapterVersion = versionMismatch ? '0.2.0' : '0.1.0';
  fs.writeFileSync(
    adapterPath,
    `
import readline from 'node:readline';
import { writeFileSync } from 'node:fs';

writeFileSync(${pidPath}, String(process.pid));
${hangAction ? `process.on('SIGTERM', () => writeFileSync(${termPath}, 'received'));` : ''}

const descriptor = {
  domainId: 'neutral-fixture',
  displayName: 'Neutral Fixture',
  version: '${adapterVersion}',
  protocolVersion: '1.0.0',
  actions: [
    { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
    { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
  ],
};

const reply = (id, result) => process.stdout.write(\`${'${'}JSON.stringify({ jsonrpc: '2.0', id, result })}\\n\`);
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'neutral-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method !== 'tools/call') return;
  if (request.params.name === 'descriptor') {
    reply(request.id, { content: [], structuredContent: descriptor, isError: false });
    return;
  }
  if (request.params.name === 'snapshot') {
    reply(request.id, {
      content: [],
      structuredContent: { domainId: 'neutral-fixture', payload: request.params.arguments.input, resources: [] },
      isError: false,
    });
    return;
  }
  if (request.params.name === 'action') {
    writeFileSync(${actionStartedPath}, 'started');
    if (${hangAction}) return;
    reply(request.id, {
      content: [],
      structuredContent: {
        domainId: 'neutral-fixture',
        action: request.params.arguments.action,
        payload: request.params.arguments.input,
        resources: [],
      },
      isError: false,
    });
  }
});
`
  );
  return adapterPath;
}

function writeElicitationFixture(root: string): string {
  const fixturePath = path.join(root, 'elicitation-fixture.mjs');
  fs.writeFileSync(
    fixturePath,
    `
import readline from 'node:readline';

const reply = (id, result) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\\n');
const pendingToolCalls = new Map();

readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'elicitation-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method === 'tools/list') {
    reply(request.id, {
      tools: [{
        name: 'request_input',
        description: 'Request a bounded fixture response.',
        inputSchema: { type: 'object', properties: {}, required: [] },
      }],
    });
    return;
  }
  if (request.method === 'tools/call') {
    const elicitationId = 'fixture-elicitation-1';
    pendingToolCalls.set(elicitationId, request.id);
    process.stdout.write(JSON.stringify({
      jsonrpc: '2.0',
      id: elicitationId,
      method: 'elicitation/create',
      params: {
        mode: 'form',
        message: 'Provide fixture input',
        requestedSchema: {
          type: 'object',
          properties: { answer: { type: 'string' } },
          required: ['answer'],
        },
      },
    }) + '\\n');
    return;
  }
  if (pendingToolCalls.has(request.id) && 'result' in request) {
    const toolCallId = pendingToolCalls.get(request.id);
    pendingToolCalls.delete(request.id);
    reply(toolCallId, {
      content: [{ type: 'text', text: 'fixture elicitation received' }],
      structuredContent: request.result,
      isError: false,
    });
  }
});
`
  );
  return fixturePath;
}

function writeFixtureRoot(
  withAdapter = false,
  versionMismatch = false,
  hangAction = false,
  openAiBaseUrl?: string,
  requireToolApproval = false,
  withElicitationFixture = false
): {
  root: string;
  workingDir: string;
  provisioningPath: string;
} {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-session-runtime-'));
  roots.push(root);
  const configDir = path.join(root, 'config');
  const workingDir = path.join(root, 'workspace');
  fs.mkdirSync(configDir, { recursive: true });
  fs.mkdirSync(workingDir, { recursive: true });
  const adapterPath = withAdapter ? writeNeutralAdapter(root, versionMismatch, hangAction) : null;
  const elicitationFixturePath = withElicitationFixture ? writeElicitationFixture(root) : null;
  fs.writeFileSync(
    path.join(configDir, 'config.yaml'),
    [
      'GOSLING_PROVIDER: openai',
      'GOSLING_MODEL: gpt-4o',
      'GOSLING_DISABLE_KEYRING: true',
      'GOSLING_DISABLE_SESSION_NAMING: true',
      ...(requireToolApproval
        ? ['GOSLING_MODE: approve']
        : withElicitationFixture
          ? ['GOSLING_MODE: auto']
          : []),
      ...(openAiBaseUrl ? [`OPENAI_BASE_URL: ${JSON.stringify(openAiBaseUrl)}`] : []),
      ...(elicitationFixturePath
        ? [
            'extensions:',
            '  elicitation_fixture:',
            '    type: stdio',
            '    name: elicitation-fixture',
            '    cmd: node',
            '    args:',
            `      - ${elicitationFixturePath}`,
            '    description: Deterministic elicitation fixture',
            '    enabled: true',
            '    timeout: 10',
          ]
        : []),
      ...(adapterPath
        ? [
            'domain_adapters:',
            '  - domainId: neutral-fixture',
            '    cmd: node',
            '    args:',
            `      - ${adapterPath}`,
            '    timeout: 10',
          ]
        : []),
      '',
    ].join('\n')
  );
  const provisioningPath = path.join(root, 'provisioning.json');
  fs.writeFileSync(
    provisioningPath,
    JSON.stringify({
      schemaVersion: 1,
      identity: {
        id: product.id,
        displayName: product.displayName,
        version: product.version,
      },
      settingsAuthority: 'main_gosling',
      protocolPolicy: { mode: 'restricted', deniedMethods: [] },
      session: {
        extensions: withElicitationFixture
          ? [{ name: 'elicitation-fixture', availableTools: ['request_input'] }]
          : [{ name: 'developer', availableTools: ['shell'] }],
      },
      ...(withAdapter
        ? {
            domainAdapter: {
              domainId: 'neutral-fixture',
              displayName: 'Neutral Fixture',
              version: '0.1.0',
              protocolVersion: '1.0.0',
              actions: [
                { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
                { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
              ],
            },
          }
        : {}),
    })
  );
  return { root, workingDir, provisioningPath };
}

function createController(
  fixture: ReturnType<typeof writeFixtureRoot>,
  build = manifest()
): ShellRuntimeController {
  execFileSync('cargo', ['build', '--bin', 'gosling'], {
    cwd: repositoryRoot,
    stdio: 'inherit',
  });
  process.env.GOSLING_BINARY = binaryPath;
  process.env.GOSLING_PATH_ROOT = fixture.root;
  process.env.GOSLING_DISABLE_KEYRING = '1';
  const controller = createShellRuntimeController({
    profile,
    manifest: build,
    provisioningPath: fixture.provisioningPath,
    diagnosticsDir: path.join(fixture.root, 'diagnostics'),
    processRegistryPath: path.join(fixture.root, 'backend-processes.json'),
    workingDir: fixture.workingDir,
    isPackaged: false,
    preloadPath: path.join(fixture.root, 'shell-preload.js'),
    sessionPartition: 'persist:gosling-shell-session-integration',
    clientName: product.id,
    clientVersion: product.version,
  });
  controllers.push(controller);
  return controller;
}

function processRegistry(root: string): unknown {
  return JSON.parse(fs.readFileSync(path.join(root, 'backend-processes.json'), 'utf8'));
}

async function eventually(predicate: () => boolean, timeoutMs = 10_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  throw new Error('timed out waiting for shell runtime state');
}

async function startFixtureOpenAiServer(
  responseMode: 'stream' | 'hold' | 'permission' | 'permission-denied' | 'elicitation' = 'stream'
): Promise<{ baseUrl: string; close(): Promise<void> }> {
  const sockets = new Set<Socket>();
  let requestCount = 0;
  const server = createServer((request, response) => {
    if (request.method !== 'POST' || request.url !== '/v1/chat/completions') {
      response.writeHead(404).end();
      return;
    }
    requestCount += 1;
    response.writeHead(200, { 'content-type': 'text/event-stream' });
    if (
      responseMode === 'permission' ||
      responseMode === 'permission-denied' ||
      responseMode === 'elicitation'
    ) {
      const command =
        responseMode === 'permission-denied'
          ? 'touch permission-denied-executed'
          : 'printf permission-fixture';
      const toolName =
        responseMode === 'elicitation' ? 'elicitation-fixture__request_input' : 'developer__shell';
      const responseChunks =
        requestCount === 1
          ? [
              `data: ${JSON.stringify({
                id: 'fixture',
                object: 'chat.completion.chunk',
                choices: [
                  {
                    index: 0,
                    delta: {
                      role: 'assistant',
                      tool_calls: [
                        {
                          index: 0,
                          id: 'call_fixture_permission',
                          type: 'function',
                          function: {
                            name: toolName,
                            arguments: JSON.stringify({ command }),
                          },
                        },
                      ],
                    },
                    finish_reason: null,
                  },
                ],
              })}`,
              'data: {"id":"fixture","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}',
              'data: [DONE]',
              '',
            ]
          : [
              `data: ${JSON.stringify({
                id: 'fixture',
                object: 'chat.completion.chunk',
                choices: [
                  {
                    index: 0,
                    delta: {
                      role: 'assistant',
                      content:
                        responseMode === 'elicitation'
                          ? 'fixture elicitation resolved'
                          : `fixture permission ${responseMode === 'permission-denied' ? 'denied' : 'resolved'}`,
                    },
                    finish_reason: null,
                  },
                ],
              })}`,
              'data: {"id":"fixture","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}',
              'data: [DONE]',
              '',
            ];
      response.end(responseChunks.join('\n\n'));
      return;
    }
    const firstChunk =
      responseMode === 'hold'
        ? 'data: {"id":"fixture","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"fixture pending"},"finish_reason":null}]}'
        : 'data: {"id":"fixture","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"fixture streamed response"},"finish_reason":null}]}';
    if (responseMode === 'hold') {
      response.write(`${firstChunk}\n\n`);
      return;
    }
    response.end(
      [
        firstChunk,
        'data: {"id":"fixture","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}',
        'data: [DONE]',
        '',
      ].join('\n\n')
    );
  });
  server.on('connection', (socket) => {
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
  });
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => resolve());
  });
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('fixture provider did not bind a TCP port');
  }

  return {
    baseUrl: `http://127.0.0.1:${address.port}/v1`,
    close: async () => {
      for (const socket of sockets) socket.destroy();
      await new Promise<void>((resolve, reject) =>
        server.close((error) => (error ? reject(error) : resolve()))
      );
    },
  };
}

function backendProcessId(root: string): number {
  const registry = processRegistry(root) as { processes?: Array<{ pid?: unknown }> };
  const pid = registry.processes?.[0]?.pid;
  if (typeof pid !== 'number' || !Number.isSafeInteger(pid) || pid <= 0) {
    throw new Error('shell backend process was not registered');
  }
  return pid;
}

function adapterProcessId(root: string): number {
  const pid = Number(fs.readFileSync(path.join(root, 'neutral-adapter.pid'), 'utf8'));
  if (!Number.isSafeInteger(pid) || pid <= 0) {
    throw new Error('neutral adapter process did not record its PID');
  }
  return pid;
}

function processIsAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function sessionCount(root: string): number {
  const databasePath = path.join(root, 'data', 'shells', namespace, 'sessions', 'sessions.db');
  if (!fs.existsSync(databasePath)) {
    return 0;
  }
  const database = new DatabaseSync(databasePath, { readOnly: true });
  try {
    return Number(database.prepare('SELECT COUNT(*) AS count FROM sessions').get()!.count);
  } finally {
    database.close();
  }
}

function startupDiagnostics(root: string): unknown {
  const directory = path.join(root, 'diagnostics');
  if (!fs.existsSync(directory)) return null;
  const file = fs
    .readdirSync(directory)
    .find((name) => name.startsWith('gosling-serve-startup-') && name.endsWith('.json'));
  return file ? JSON.parse(fs.readFileSync(path.join(directory, file), 'utf8')) : null;
}

async function stopController(controller: ShellRuntimeController): Promise<void> {
  await controller.stop(controller.read().generation);
  const index = controllers.indexOf(controller);
  if (index >= 0) {
    controllers.splice(index, 1);
  }
}

afterEach(async () => {
  for (const controller of controllers.splice(0)) {
    await controller.stop(controller.read().generation);
  }
  delete process.env.GOSLING_BINARY;
  delete process.env.GOSLING_PATH_ROOT;
  delete process.env.GOSLING_DISABLE_KEYRING;
  for (const root of roots.splice(0)) {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

describe('shell session runtime integration', () => {
  it('streams a deterministic provider response through the bounded main-owned projection', async () => {
    const provider = await startFixtureOpenAiServer();
    try {
      const fixture = writeFixtureRoot(false, false, false, provider.baseUrl);
      const controller = createController(fixture);
      const started = await controller.start();
      expect(started, JSON.stringify(controller.getStartupDiagnostics(), null, 2)).toMatchObject({
        name: 'ready',
        generation: 1,
      });
      expect(controller.read().runtimeNamespace).toBe(namespace);
      const sessions = controller.getSessionController()!;
      const session = await sessions.create(1);
      const updates: unknown[] = [];
      controller.onSessionUpdated((update) => updates.push(update));

      sessions.submit({ generation: 1, sessionId: session.sessionId, text: 'stream fixture' });
      await eventually(() =>
        updates.some(
          (update) =>
            typeof update === 'object' &&
            update !== null &&
            'kind' in update &&
            update.kind === 'stream' &&
            'stream' in update &&
            typeof update.stream === 'object' &&
            update.stream !== null &&
            'type' in update.stream &&
            update.stream.type === 'content' &&
            'text' in update.stream &&
            update.stream.text === 'fixture streamed response'
        )
      );
      await eventually(() => controller.getSessionController()!.read().promptAttempt === null);
      expect(controller.read().session).toMatchObject({
        sessionId: session.sessionId,
        status: 'active',
        promptAttempt: null,
      });
      await stopController(controller);
    } finally {
      await provider.close();
    }
  });

  it('cancels an in-flight deterministic provider prompt without retaining its attempt', async () => {
    const provider = await startFixtureOpenAiServer('hold');
    try {
      const fixture = writeFixtureRoot(false, false, false, provider.baseUrl);
      const controller = createController(fixture);
      await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
      const sessions = controller.getSessionController()!;
      const session = await sessions.create(1);
      const updates: Array<{ kind: string; stream?: { type?: string; text?: string } }> = [];
      controller.onSessionUpdated((update) => updates.push(update));

      const submitted = sessions.submit({
        generation: 1,
        sessionId: session.sessionId,
        text: 'cancel fixture',
      });
      await eventually(() =>
        updates.some(
          (update) => update.kind === 'stream' && update.stream?.text === 'fixture pending'
        )
      );
      await sessions.cancel({
        generation: 1,
        sessionId: session.sessionId,
        promptAttemptId: submitted.promptAttemptId,
      });

      await eventually(() => updates.some((update) => update.kind === 'cancelled'));
      expect(sessions.read().promptAttempt).toBeNull();
      await stopController(controller);
    } finally {
      await provider.close();
    }
  });

  it('publishes a terminal prompt failure before going offline when the live child exits', async () => {
    const provider = await startFixtureOpenAiServer('hold');
    try {
      const fixture = writeFixtureRoot(false, false, false, provider.baseUrl);
      const controller = createController(fixture);
      await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
      const sessions = controller.getSessionController()!;
      const session = await sessions.create(1);
      const updates: Array<{ kind: string; stream?: { type?: string; text?: string } }> = [];
      controller.onSessionUpdated((update) => updates.push(update));

      sessions.submit({ generation: 1, sessionId: session.sessionId, text: 'disconnect fixture' });
      await eventually(() =>
        updates.some(
          (update) => update.kind === 'stream' && update.stream?.text === 'fixture pending'
        )
      );
      process.kill(backendProcessId(fixture.root), 'SIGTERM');

      await eventually(() => updates.some((update) => update.kind === 'failed'));
      await eventually(() => controller.read().name === 'offline');
      expect(controller.read()).toMatchObject({
        name: 'offline',
        reasonCode: 'BACKEND_EXITED',
        session: null,
        pendingInteractions: [],
      });
      await expect(controller.retry(1)).resolves.toMatchObject({ name: 'ready', generation: 2 });
      await expect(
        controller.getSessionController()!.resume(2, session.sessionId)
      ).resolves.toMatchObject({
        resumeKind: 'resumed',
        resumeIntegrity: 'uncertain',
      });
      await stopController(controller);
    } finally {
      await provider.close();
    }
  });

  it('holds a live tool call for a main-owned permission decision and resumes the prompt', async () => {
    const provider = await startFixtureOpenAiServer('permission');
    try {
      const fixture = writeFixtureRoot(false, false, false, provider.baseUrl, true);
      const controller = createController(fixture);
      await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
      const sessions = controller.getSessionController()!;
      const session = await sessions.create(1);
      const interactions = [] as NonNullable<
        ReturnType<typeof controller.read>['pendingInteractions']
      >;
      const updates: Array<{ kind: string; stream?: { type?: string; text?: string } }> = [];
      controller.onInteractionRequested((interaction) => interactions.push(interaction));
      controller.onSessionUpdated((update) => updates.push(update));

      sessions.submit({
        generation: 1,
        sessionId: session.sessionId,
        text: 'request the permission fixture',
      });
      await eventually(
        () =>
          interactions.length === 1 ||
          updates.some((update) => update.kind === 'failed' || update.kind === 'completed')
      );
      expect(interactions, JSON.stringify(updates)).toHaveLength(1);
      const pending = interactions[0];
      expect(pending).toMatchObject({
        kind: 'permission',
        generation: 1,
        sessionId: session.sessionId,
        summary: { toolTitle: 'developer: shell', allowOnce: true, deny: true },
      });
      expect(JSON.stringify(pending)).not.toContain('permission-fixture');
      expect(controller.read().pendingInteractions).toEqual([pending]);

      controller.getInteractionController()!.respondPermission({
        actionId: pending.actionId,
        generation: pending.generation,
        sessionId: pending.sessionId,
        allowOnce: true,
      });
      await eventually(() => controller.read().pendingInteractions.length === 0);
      await eventually(() =>
        updates.some(
          (update) =>
            update.kind === 'stream' && update.stream?.text === 'fixture permission resolved'
        )
      );
      await eventually(() => sessions.read().promptAttempt === null);
      await stopController(controller);
    } finally {
      await provider.close();
    }
  });

  it('denies a live tool call without executing its requested side effect', async () => {
    const provider = await startFixtureOpenAiServer('permission-denied');
    try {
      const fixture = writeFixtureRoot(false, false, false, provider.baseUrl, true);
      const controller = createController(fixture);
      await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
      const sessions = controller.getSessionController()!;
      const session = await sessions.create(1);
      const interactions = [] as NonNullable<
        ReturnType<typeof controller.read>['pendingInteractions']
      >;
      const updates: Array<{ kind: string; stream?: { type?: string; text?: string } }> = [];
      controller.onInteractionRequested((interaction) => interactions.push(interaction));
      controller.onSessionUpdated((update) => updates.push(update));

      sessions.submit({
        generation: 1,
        sessionId: session.sessionId,
        text: 'deny the permission fixture',
      });
      await eventually(
        () =>
          interactions.length === 1 ||
          updates.some((update) => update.kind === 'failed' || update.kind === 'completed')
      );
      expect(interactions, JSON.stringify(updates)).toHaveLength(1);
      const pending = interactions[0];
      controller.getInteractionController()!.respondPermission({
        actionId: pending.actionId,
        generation: pending.generation,
        sessionId: pending.sessionId,
        allowOnce: false,
      });

      await eventually(() => controller.read().pendingInteractions.length === 0);
      await eventually(() =>
        updates.some(
          (update) =>
            update.kind === 'stream' && update.stream?.text === 'fixture permission denied'
        )
      );
      await eventually(() => sessions.read().promptAttempt === null);
      expect(fs.existsSync(path.join(fixture.workingDir, 'permission-denied-executed'))).toBe(
        false
      );
      await stopController(controller);
    } finally {
      await provider.close();
    }
  });

  it('holds a live MCP elicitation for a main-owned form submission and resumes the prompt', async () => {
    const provider = await startFixtureOpenAiServer('elicitation');
    try {
      const fixture = writeFixtureRoot(false, false, false, provider.baseUrl, false, true);
      const controller = createController(fixture);
      await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
      const sessions = controller.getSessionController()!;
      const session = await sessions.create(1);
      const interactions = [] as NonNullable<
        ReturnType<typeof controller.read>['pendingInteractions']
      >;
      const updates: Array<{ kind: string; stream?: { type?: string; text?: string } }> = [];
      controller.onInteractionRequested((interaction) => interactions.push(interaction));
      controller.onSessionUpdated((update) => updates.push(update));

      sessions.submit({
        generation: 1,
        sessionId: session.sessionId,
        text: 'request the elicitation fixture',
      });
      await eventually(
        () =>
          interactions.length === 1 ||
          updates.some((update) => update.kind === 'failed' || update.kind === 'completed')
      );
      expect(interactions, JSON.stringify(updates)).toHaveLength(1);
      const pending = interactions[0];
      expect(pending).toMatchObject({
        kind: 'elicitation',
        generation: 1,
        sessionId: session.sessionId,
        summary: { message: 'Provide fixture input', fields: ['answer'] },
      });
      expect(controller.read().pendingInteractions).toEqual([pending]);

      controller.getInteractionController()!.respondElicitation({
        actionId: pending.actionId,
        generation: pending.generation,
        sessionId: pending.sessionId,
        action: 'submit',
        fields: { answer: 'fixture value' },
      });
      await eventually(() => controller.read().pendingInteractions.length === 0);
      await eventually(() =>
        updates.some(
          (update) =>
            update.kind === 'stream' && update.stream?.text === 'fixture elicitation resolved'
        )
      );
      await eventually(() => sessions.read().promptAttempt === null);
      await stopController(controller);
    } finally {
      await provider.close();
    }
  });

  it('creates after compatibility and resumes after a backend restart', async () => {
    const fixture = writeFixtureRoot();
    const first = createController(fixture);
    await expect(first.start()).resolves.toMatchObject({ name: 'ready' });
    const created = await first.getAcp()!.createSession();
    expect(created.workingDir).toBe(fixture.workingDir);
    await stopController(first);

    const second = createController(fixture);
    await expect(second.start()).resolves.toMatchObject({ name: 'ready' });
    await expect(second.getAcp()!.resumeSession(created.sessionId)).resolves.toEqual({
      ...created,
      resumeIntegrity: 'uncertain',
    });
    await stopController(second);

    expect(sessionCount(fixture.root)).toBe(1);
    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
  });

  it('does not create durable session state when compatibility fails', async () => {
    const fixture = writeFixtureRoot();
    const controller = createController(fixture, manifest('9.9.9'));
    await expect(controller.start()).resolves.toMatchObject({
      name: 'incompatible',
      reasonCode: 'CORE_MISMATCH',
    });
    expect(controller.getAcp()).toBeNull();
    await stopController(controller);

    expect(sessionCount(fixture.root)).toBe(0);
    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
  });

  it('fails a live adapter version mismatch before ready and reaps its child process', async () => {
    const fixture = writeFixtureRoot(true, true);
    const controller = createController(fixture, adapterManifest());

    await expect(controller.start()).resolves.toMatchObject({
      name: 'incompatible',
      generation: 1,
      reasonCode: 'ADAPTER_DESCRIPTOR_MISMATCH',
    });
    expect(JSON.stringify(startupDiagnostics(fixture.root))).toContain(
      'ADAPTER_DESCRIPTOR_MISMATCH'
    );
    await eventually(() => fs.existsSync(path.join(fixture.root, 'neutral-adapter.pid')));
    const adapterPid = adapterProcessId(fixture.root);
    await eventually(() => !processIsAlive(adapterPid));
    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
  });

  it('fences a prompt attempt, recovers from a real child exit, and requires explicit resume', async () => {
    const fixture = writeFixtureRoot();
    const controller = createController(fixture);
    await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });

    const sessions = controller.getSessionController();
    expect(sessions).not.toBeNull();
    const created = await sessions!.create(1);
    const updates: string[] = [];
    controller.onSessionUpdated((update) => updates.push(update.kind));
    const submitted = sessions!.submit({
      generation: 1,
      sessionId: created.sessionId,
      text: 'neutral no-credential prompt transport check',
    });
    expect(submitted.promptAttemptId).toBeTruthy();
    await eventually(() => updates.some((kind) => kind === 'completed' || kind === 'failed'));
    expect(controller.read()).toMatchObject({ name: 'ready', session: { status: 'active' } });

    process.kill(backendProcessId(fixture.root), 'SIGTERM');
    await eventually(() => controller.read().name === 'offline');
    expect(controller.read().session).toBeNull();

    await expect(controller.retry(1)).resolves.toMatchObject({ name: 'ready', generation: 2 });
    const resumed = await controller.getSessionController()!.resume(2, created.sessionId);
    expect(resumed).toMatchObject({
      sessionId: created.sessionId,
      status: 'active',
      resumeKind: 'resumed',
      resumeIntegrity: 'clean',
    });
    await stopController(controller);

    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
  });

  it('negotiates and confirms a neutral adapter mutation through the authenticated ACP connection', async () => {
    const fixture = writeFixtureRoot(true);
    const controller = createController(fixture, adapterManifest());
    const state = await controller.start();
    expect(state, JSON.stringify(startupDiagnostics(fixture.root), null, 2)).toMatchObject({
      name: 'ready',
      generation: 1,
    });
    const acp = controller.getAcp()!;
    expect(acp.domainAdapter).toEqual({
      descriptorId: 'neutral-fixture',
      protocolVersion: '1.0.0',
      actions: ['inspect', 'toggle'],
    });
    await eventually(() => fs.existsSync(path.join(fixture.root, 'neutral-adapter.pid')));
    const adapterPid = adapterProcessId(fixture.root);
    expect(processIsAlive(adapterPid)).toBe(true);
    const session = await acp.createSession();
    await expect(acp.domainSnapshot({ input: { scope: 'neutral' } })).resolves.toMatchObject({
      domainId: 'neutral-fixture',
      payload: { scope: 'neutral' },
    });
    const pending = await acp.domainAction({
      sessionId: session.sessionId,
      generation: 1,
      action: 'toggle',
      input: { enabled: true },
    });
    expect(pending.confirmationActionId).toEqual(expect.any(String));
    const confirmed = await acp.confirmDomainAction({
      sessionId: session.sessionId,
      generation: 1,
      actionId: pending.confirmationActionId!,
      approve: true,
    });
    expect(confirmed).toMatchObject({
      status: 'approved',
      result: {
        domainId: 'neutral-fixture',
        action: 'toggle',
        payload: { enabled: true },
      },
    });
    await stopController(controller);

    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
    await eventually(() => !processIsAlive(adapterPid));
  });

  it('projects an idle adapter exit into the safe runtime snapshot and cleans the owned server', async () => {
    const fixture = writeFixtureRoot(true);
    const controller = createController(fixture, adapterManifest());
    await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
    await eventually(() => fs.existsSync(path.join(fixture.root, 'neutral-adapter.pid')));
    const adapterPid = adapterProcessId(fixture.root);

    process.kill(adapterPid, 'SIGTERM');
    await eventually(() => controller.read().adapter?.status === 'crashed');
    expect(controller.read().adapter).toMatchObject({
      descriptorId: 'neutral-fixture',
      status: 'crashed',
    });

    await stopController(controller);
    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
  });

  it('fails an in-flight adapter action and projects the resulting crash', async () => {
    const fixture = writeFixtureRoot(true, false, true);
    const controller = createController(fixture, adapterManifest());
    await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
    await eventually(() => fs.existsSync(path.join(fixture.root, 'neutral-adapter.pid')));
    const adapterPid = adapterProcessId(fixture.root);
    const acp = controller.getAcp()!;
    const session = await acp.createSession();
    const action = acp.domainAction({
      sessionId: session.sessionId,
      generation: 1,
      action: 'inspect',
      input: { mode: 'hang' },
    });
    await eventually(() =>
      fs.existsSync(path.join(fixture.root, 'neutral-adapter-action.started'))
    );

    process.kill(adapterPid, 'SIGKILL');
    await expect(action).rejects.toThrow();
    await eventually(() => controller.read().adapter?.status === 'crashed');
    await stopController(controller);
    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
  });

  it('restarts the adapter with the backend and restores its declared capability', async () => {
    const fixture = writeFixtureRoot(true);
    const controller = createController(fixture, adapterManifest());
    await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
    await eventually(() => fs.existsSync(path.join(fixture.root, 'neutral-adapter.pid')));
    const firstAdapterPid = adapterProcessId(fixture.root);

    process.kill(backendProcessId(fixture.root), 'SIGTERM');
    await eventually(() => controller.read().name === 'offline');
    await eventually(() => !processIsAlive(firstAdapterPid));

    await expect(controller.retry(1)).resolves.toMatchObject({ name: 'ready', generation: 2 });
    await eventually(() => adapterProcessId(fixture.root) !== firstAdapterPid);
    expect(controller.read().adapter).toMatchObject({
      descriptorId: 'neutral-fixture',
      protocolVersion: '1.0.0',
      status: 'ready',
    });
    await expect(
      controller.getAcp()!.domainSnapshot({ input: { scope: 'restarted' } })
    ).resolves.toMatchObject({
      domainId: 'neutral-fixture',
      payload: { scope: 'restarted' },
    });

    await stopController(controller);
    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
  });

  it('reaps a non-cooperative in-flight adapter during shell cleanup', async () => {
    const fixture = writeFixtureRoot(true, false, true);
    const controller = createController(fixture, adapterManifest());
    await expect(controller.start()).resolves.toMatchObject({ name: 'ready', generation: 1 });
    await eventually(() => fs.existsSync(path.join(fixture.root, 'neutral-adapter.pid')));
    const adapterPid = adapterProcessId(fixture.root);
    const acp = controller.getAcp()!;
    const session = await acp.createSession();
    void acp
      .domainAction({
        sessionId: session.sessionId,
        generation: 1,
        action: 'inspect',
        input: { mode: 'hang' },
      })
      .catch(() => {});
    await eventually(() =>
      fs.existsSync(path.join(fixture.root, 'neutral-adapter-action.started'))
    );

    await expect(controller.stop(1)).resolves.toMatchObject({ name: 'stopped', generation: 1 });
    expect(processRegistry(fixture.root)).toEqual({ version: 1, processes: [] });
    expect(fs.existsSync(path.join(fixture.root, 'neutral-adapter-sigterm.received'))).toBe(true);
    await eventually(() => !processIsAlive(adapterPid));
  });
});
