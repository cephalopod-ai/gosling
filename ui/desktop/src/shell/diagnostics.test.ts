import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import {
  buildShellDiagnostics,
  serializeShellDiagnostics,
  writeShellDiagnostics,
} from './diagnostics';
import type { ShellBuildManifest } from './profile';

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

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
const manifest = {
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
    requiredMethods: [],
  },
} satisfies ShellBuildManifest;

function bundle() {
  return buildShellDiagnostics({
    generatedAt: '2026-08-13T00:00:00Z',
    manifest,
    lifecycle: {
      generation: 1,
      name: 'offline',
      enteredAt: 'now',
      reasonCode: 'STARTUP_FAILED',
      allowedActions: ['retry', 'diagnostics'],
    },
    startup: {
      attemptId: 'attempt',
      startedAt: 'now',
      binaryPath: '/Applications/Gosling/gosling',
      workingDir: '/Users/person/private-work',
      httpBaseUrl: 'http://127.0.0.1:7777',
      readinessUrl: 'http://127.0.0.1:7777/status',
      statusUrl: 'http://127.0.0.1:7777/status',
      healthUrl: 'http://127.0.0.1:7777/health',
      acpUrl: 'ws://127.0.0.1:7777/acp?token=sentinel-secret',
      pid: 42,
      healthCheckSucceeded: false,
      childExitCode: 17,
      childExitSignal: null,
      stderrTail: [
        'authorization=Bearer sentinel-secret',
        'password=sentinel-password',
        '/Users/person/private-work/file',
      ],
      events: [
        {
          name: 'failed',
          at: 'now',
          elapsedMs: 1,
          details: { token: 'sentinel-token', path: '/Users/person/private-work' },
        },
      ],
    },
    exitDetails: { code: 17, signal: null },
    processRegistryPath: '/does/not/exist',
    home: '/Users/person',
  });
}

describe('shell diagnostics', () => {
  it('exports only bounded allowlisted identity and redacts secret/path/network sentinels', () => {
    const serialized = serializeShellDiagnostics(bundle());
    expect(serialized).toContain('"profileHash"');
    expect(serialized).toContain('gosling');
    expect(serialized).not.toContain('sentinel-secret');
    expect(serialized).not.toContain('sentinel-password');
    expect(serialized).not.toContain('sentinel-token');
    expect(serialized).not.toContain('/Users/person');
    expect(serialized).not.toContain('127.0.0.1');
    expect(serialized).not.toContain('runtimeNamespace');
    expect(Buffer.byteLength(serialized, 'utf8')).toBeLessThan(1024 * 1024);
  });

  it('writes atomically with owner-private permissions and refuses overwrite', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-diagnostics-'));
    roots.push(root);
    const file = path.join(root, 'diagnostics.json');
    writeShellDiagnostics(file, serializeShellDiagnostics(bundle()));
    expect(fs.statSync(file).mode & 0o777).toBe(0o600);
    expect(fs.readdirSync(root)).toEqual(['diagnostics.json']);
    expect(() => writeShellDiagnostics(file, '{}')).toThrow('refuses to overwrite');
  });

  it('removes a temporary file after a failed atomic write', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-diagnostics-'));
    roots.push(root);
    const directory = path.join(root, 'directory-target');
    fs.mkdirSync(directory);
    expect(() => writeShellDiagnostics(directory, '{}')).toThrow();
    expect(fs.readdirSync(root)).toEqual(['directory-target']);
  });
});
