import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
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

function writeFixtureRoot(): { root: string; workingDir: string; provisioningPath: string } {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-session-runtime-'));
  roots.push(root);
  const configDir = path.join(root, 'config');
  const workingDir = path.join(root, 'workspace');
  fs.mkdirSync(configDir, { recursive: true });
  fs.mkdirSync(workingDir, { recursive: true });
  fs.writeFileSync(
    path.join(configDir, 'config.yaml'),
    'GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\nGOSLING_DISABLE_KEYRING: true\n'
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
        extensions: [{ name: 'developer', availableTools: ['read_file'] }],
      },
    })
  );
  return { root, workingDir, provisioningPath };
}

function createController(
  fixture: ReturnType<typeof writeFixtureRoot>,
  build = manifest()
): ShellRuntimeController {
  if (!fs.existsSync(binaryPath)) {
    execFileSync('cargo', ['build', '--bin', 'gosling'], {
      cwd: repositoryRoot,
      stdio: 'inherit',
    });
  }
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
  it('creates after compatibility and resumes after a backend restart', async () => {
    const fixture = writeFixtureRoot();
    const first = createController(fixture);
    await expect(first.start()).resolves.toMatchObject({ name: 'ready' });
    const created = await first.getAcp()!.createSession();
    expect(created.workingDir).toBe(fixture.workingDir);
    await stopController(first);

    const second = createController(fixture);
    await expect(second.start()).resolves.toMatchObject({ name: 'ready' });
    await expect(second.getAcp()!.resumeSession(created.sessionId)).resolves.toEqual(created);
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
});
