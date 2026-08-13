import { describe, expect, it } from 'vitest';
import { checkShellCompatibility } from './compatibility';
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
  macosBundleId: 'io.github.repo_makeover.gosling.fixture.a',
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
    requiredTargets: ['macos-arm64'],
  },
  update: { enabled: false, channel: 'fixture-disabled' },
  distribution: { publishable: false, artifactPrefix: 'fixture-a', signingPolicy: 'none' },
};
const manifest: ShellBuildManifest = {
  schemaVersion: 1,
  profileSchemaVersion: 1,
  profileHash: 'a'.repeat(64),
  product,
  target: 'macos-arm64',
  platform: 'macos',
  architecture: 'arm64',
  sourceClean: false,
  compatibility: {
    goslingVersion: '0.1.0',
    goslingRevision: 'b'.repeat(40),
    provisioningSchemaVersion: 1,
    handoffSchemaVersion: 1,
    requiredMethods: methods,
  },
};

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function input() {
  return {
    profile: clone(profile),
    manifest: clone(manifest),
    runtime: {
      identity: clone(product),
      coreVersion: '0.1.0',
      availableMethods: [...methods],
    },
    provisioning: { schemaVersion: 1, valid: true },
  };
}

describe('shell compatibility', () => {
  it('accepts the exact bundled-core, identity, schema, method, and provisioning contract', () => {
    expect(checkShellCompatibility(input())).toEqual({ compatible: true });
  });

  it.each([
    [
      'PROFILE_SCHEMA_UNSUPPORTED',
      (value: ReturnType<typeof input>) => {
        value.manifest.profileSchemaVersion = 2 as 1;
      },
    ],
    [
      'IDENTITY_MISMATCH',
      (value: ReturnType<typeof input>) => {
        value.runtime.identity.id = 'other';
      },
    ],
    [
      'CORE_MISMATCH',
      (value: ReturnType<typeof input>) => {
        value.runtime.coreVersion = '0.2.0';
      },
    ],
    [
      'CORE_MISMATCH',
      (value: ReturnType<typeof input>) => {
        value.manifest.compatibility.goslingRevision = 'invalid';
      },
    ],
    [
      'PROVISIONING_SCHEMA_UNSUPPORTED',
      (value: ReturnType<typeof input>) => {
        value.provisioning.schemaVersion = 2;
      },
    ],
    [
      'METHOD_UNAVAILABLE',
      (value: ReturnType<typeof input>) => {
        value.runtime.availableMethods.pop();
      },
    ],
    [
      'PROVISIONING_INVALID',
      (value: ReturnType<typeof input>) => {
        value.provisioning.valid = false;
      },
    ],
  ])('fails closed with %s before session use', (code, mutate) => {
    const value = input();
    mutate(value);
    expect(checkShellCompatibility(value)).toMatchObject({ compatible: false, code });
  });

  it('limits identity failures to the server-fixed identity triplet', () => {
    const value = input();
    value.runtime.identity.id = 'other';
    expect(checkShellCompatibility(value)).toEqual({
      compatible: false,
      code: 'IDENTITY_MISMATCH',
      expected: {
        id: product.id,
        displayName: product.displayName,
        version: product.version,
      },
      actual: {
        manifest: {
          id: product.id,
          displayName: product.displayName,
          version: product.version,
        },
        runtime: {
          id: 'other',
          displayName: product.displayName,
          version: product.version,
        },
      },
    });
  });

  it('compares methods as a set and reports a sorted non-secret actual list', () => {
    const value = input();
    value.runtime.availableMethods = ['z-method', ...methods.slice(0, 2), 'a-method'];
    expect(checkShellCompatibility(value)).toEqual({
      compatible: false,
      code: 'METHOD_UNAVAILABLE',
      expected: methods,
      actual: ['a-method', ...methods.slice(0, 2), 'z-method'].sort(),
    });
  });
});
