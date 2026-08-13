import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type { GoslingServeStartupDiagnostics } from '../startupDiagnostics';
import type { ShellLifecycleState } from './lifecycle';
import type { ShellBuildManifest } from './profile';

const MAX_DIAGNOSTIC_BYTES = 1024 * 1024;
const MAX_EVENTS = 100;
const MAX_STRING_LENGTH = 2 * 1024;
const SENSITIVE_KEY = /authorization|cookie|credential|password|private.?key|secret|token/i;
const SENSITIVE_ASSIGNMENT =
  /((?:authorization|cookie|credential|password|private[_-]?key|secret|token)\s*[:=]\s*)[^,;]+/gi;

export interface ShellDiagnosticBundle {
  schemaVersion: 1;
  generatedAt: string;
  product: { id: string; displayName: string; version: string; profileHash: string };
  package: {
    goslingVersion: string;
    goslingRevision: string;
    target: string;
  };
  lifecycle: ShellLifecycleState;
  child: { exitCode: number | null; exitSignal: string | null } | null;
  startup: unknown;
  cleanup: { processRegistryExists: boolean };
}

function redactString(value: string, home: string): string {
  const redactedHome = home ? value.split(home).join('<home>') : value;
  return redactedHome.replace(SENSITIVE_ASSIGNMENT, '$1[redacted]').slice(0, MAX_STRING_LENGTH);
}

function redactValue(
  value: unknown,
  home: string,
  key = '',
  seen = new WeakSet<object>()
): unknown {
  if (SENSITIVE_KEY.test(key)) {
    return '[redacted]';
  }
  if (typeof value === 'string') {
    return redactString(value, home);
  }
  if (value === null || typeof value !== 'object') {
    return value;
  }
  if (seen.has(value)) {
    return '[circular]';
  }
  seen.add(value);
  if (Array.isArray(value)) {
    return value.slice(0, MAX_EVENTS).map((item) => redactValue(item, home, key, seen));
  }
  return Object.fromEntries(
    Object.entries(value).map(([childKey, child]) => [
      childKey,
      redactValue(child, home, childKey, seen),
    ])
  );
}

export function buildShellDiagnostics(input: {
  generatedAt: string;
  manifest: ShellBuildManifest;
  lifecycle: ShellLifecycleState;
  startup: GoslingServeStartupDiagnostics | null;
  exitDetails: { code: number | null; signal: string | null } | null;
  processRegistryPath: string;
  home?: string;
}): ShellDiagnosticBundle {
  const startup = input.startup
    ? {
        ...input.startup,
        binaryPath: input.startup.binaryPath ? path.basename(input.startup.binaryPath) : null,
        workingDir: '<redacted>',
        httpBaseUrl: null,
        readinessUrl: null,
        statusUrl: null,
        healthUrl: null,
        acpUrl: null,
        stderrTail: input.startup.stderrTail.slice(-80),
        events: input.startup.events.slice(-MAX_EVENTS),
      }
    : null;
  return redactValue(
    {
      schemaVersion: 1,
      generatedAt: input.generatedAt,
      product: {
        id: input.manifest.product.id,
        displayName: input.manifest.product.displayName,
        version: input.manifest.product.version,
        profileHash: input.manifest.profileHash,
      },
      package: {
        goslingVersion: input.manifest.compatibility.goslingVersion,
        goslingRevision: input.manifest.compatibility.goslingRevision,
        target: input.manifest.target,
      },
      lifecycle: input.lifecycle,
      child: input.exitDetails
        ? { exitCode: input.exitDetails.code, exitSignal: input.exitDetails.signal }
        : null,
      startup,
      cleanup: { processRegistryExists: fs.existsSync(input.processRegistryPath) },
    },
    input.home ?? os.homedir()
  ) as ShellDiagnosticBundle;
}

export function serializeShellDiagnostics(bundle: ShellDiagnosticBundle): string {
  const serialized = `${JSON.stringify(bundle, null, 2)}\n`;
  if (Buffer.byteLength(serialized, 'utf8') > MAX_DIAGNOSTIC_BYTES) {
    throw new Error('diagnostic bundle exceeds the 1 MiB limit');
  }
  return serialized;
}

export function writeShellDiagnostics(file: string, serialized: string): void {
  if (fs.existsSync(file)) {
    throw new Error('diagnostic export refuses to overwrite an existing file');
  }
  const directory = path.dirname(file);
  fs.mkdirSync(directory, { recursive: true });
  const temporary = path.join(directory, `.${path.basename(file)}.${process.pid}.tmp`);
  let descriptor: number | null = null;
  try {
    descriptor = fs.openSync(temporary, 'wx', 0o600);
    fs.writeFileSync(descriptor, serialized, 'utf8');
    fs.fsyncSync(descriptor);
    fs.closeSync(descriptor);
    descriptor = null;
    fs.renameSync(temporary, file);
    fs.chmodSync(file, 0o600);
  } catch (error) {
    if (descriptor !== null) {
      fs.closeSync(descriptor);
    }
    fs.rmSync(temporary, { force: true });
    throw error;
  }
}
