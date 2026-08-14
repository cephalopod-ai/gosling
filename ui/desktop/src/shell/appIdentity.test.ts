import path from 'node:path';
import { describe, expect, it, vi } from 'vitest';
import { applyShellAppIdentity, deriveShellAppIdentity } from './appIdentity';
import type { ShellProductIdentity } from './profile';

const product: ShellProductIdentity = {
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

describe('shell app identity', () => {
  it('derives every local path and browser partition from the product identity', () => {
    const identity = deriveShellAppIdentity(product, {
      appData: '/app-data',
      cache: '/cache',
      temp: '/temp',
    });
    const userData = path.join('/app-data', product.id);
    expect(identity).toEqual({
      name: product.displayName,
      protocolScheme: product.protocolScheme,
      sessionPartition: `persist:gosling-shell-${product.id}`,
      paths: {
        userData,
        sessionData: path.join(userData, 'session'),
        cache: path.join('/cache', product.id),
        temp: path.join('/temp', product.id),
        logs: path.join(userData, 'logs'),
        diagnostics: path.join(userData, 'diagnostics'),
        processRegistry: path.join(userData, 'backend-processes.json'),
        localSettings: path.join(userData, 'shell-settings.json'),
      },
    });
  });

  it('applies identity before callers can acquire readiness or a lock', () => {
    const calls: string[] = [];
    const adapter = {
      getPath: vi.fn((name: 'appData' | 'sessionData' | 'temp') => `/${name}`),
      setName: vi.fn((name: string) => calls.push(`name:${name}`)),
      setPath: vi.fn((name: string, value: string) => calls.push(`path:${name}:${value}`)),
    };
    const identity = applyShellAppIdentity(adapter, product);
    expect(calls[0]).toBe(`name:${product.displayName}`);
    expect(calls.slice(1).map((call) => call.split(':')[1])).toEqual([
      'userData',
      'sessionData',
      'temp',
      'logs',
    ]);
    expect(identity.paths.userData).toBe(path.join('/appData', product.id));
    expect(identity.paths.cache).toBe(path.join('/sessionData', product.id));
  });

  it('keeps two shell identities disjoint across every local path and partition', () => {
    const a = deriveShellAppIdentity(product, {
      appData: '/app-data',
      cache: '/cache',
      temp: '/temp',
    });
    const b = deriveShellAppIdentity(
      {
        ...product,
        id: 'gosling-shell-fixture-b',
        displayName: 'Gosling Shell Fixture B',
        protocolScheme: 'gosling-fixture-b',
      },
      { appData: '/app-data', cache: '/cache', temp: '/temp' }
    );
    expect(a.protocolScheme).not.toBe(b.protocolScheme);
    expect(a.sessionPartition).not.toBe(b.sessionPartition);
    for (const key of Object.keys(a.paths) as Array<keyof typeof a.paths>) {
      expect(a.paths[key]).not.toBe(b.paths[key]);
    }
  });
});
