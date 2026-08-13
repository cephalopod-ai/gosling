import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { loadShellResources } from './resources';

const roots: string[] = [];

afterEach(() => {
  for (const root of roots.splice(0)) {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (value && typeof value === 'object') {
    return `{${Object.entries(value)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, child]) => `${JSON.stringify(key)}:${canonicalJson(child)}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-resources-'));
  roots.push(root);
  const dev = path.join(root, 'dev');
  const packaged = path.join(root, 'packaged');
  fs.mkdirSync(dev);
  fs.mkdirSync(packaged);
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
  const profile = {
    schemaVersion: 1,
    product,
    provisioningPath: 'fixture/provisioning.json',
    compatibility: {
      goslingVersion: '0.1.0',
      goslingRevision: 'current',
      provisioningSchemaVersion: 1,
      handoffSchemaVersion: 1,
      requiredMethods: ['method'],
    },
    assets: { root: 'assets', iconBase: 'assets/icon', requiredTargets: ['linux-x64'] },
    update: { enabled: false, channel: 'disabled' },
    distribution: { publishable: false, artifactPrefix: 'fixture', signingPolicy: 'none' },
  };
  const manifest = {
    schemaVersion: 1,
    profileSchemaVersion: 1,
    profileHash: crypto.createHash('sha256').update(canonicalJson(profile)).digest('hex'),
    product,
    target: 'linux-x64',
    platform: 'linux',
    architecture: 'x64',
    sourceClean: false,
    compatibility: {
      goslingVersion: '0.1.0',
      goslingRevision: 'a'.repeat(40),
      provisioningSchemaVersion: 1,
      handoffSchemaVersion: 1,
      requiredMethods: ['method'],
    },
  };
  fs.writeFileSync(path.join(dev, 'profile-source.json'), JSON.stringify(profile));
  fs.writeFileSync(path.join(dev, 'manifest-source.json'), JSON.stringify(manifest));
  fs.writeFileSync(path.join(dev, 'provisioning-source.json'), '{}');
  fs.writeFileSync(path.join(packaged, 'profile.json'), JSON.stringify(profile));
  fs.writeFileSync(path.join(packaged, 'manifest.json'), JSON.stringify(manifest));
  fs.writeFileSync(path.join(packaged, 'provisioning.json'), '{}');
  const files = {
    profileFileName: 'profile.json',
    manifestFileName: 'manifest.json',
    provisioningFileName: 'provisioning.json',
    developmentProfilePath: path.join(dev, 'profile-source.json'),
    developmentManifestPath: path.join(dev, 'manifest-source.json'),
    developmentProvisioningPath: path.join(dev, 'provisioning-source.json'),
  };
  return { files, manifest, packaged, profile };
}

describe('shell resources', () => {
  it('uses compile-time development paths without runtime profile selection', () => {
    const value = fixture();
    const loaded = loadShellResources({
      isPackaged: false,
      resourcesPath: value.packaged,
      files: value.files,
    });
    expect(loaded.profile).toEqual(value.profile);
    expect(loaded.manifest).toEqual(value.manifest);
    expect(loaded.profilePath).toBe(value.files.developmentProfilePath);
    expect(loaded.provisioningPath).toBe(value.files.developmentProvisioningPath);
  });

  it('uses only copied basenames for packaged resources', () => {
    const value = fixture();
    const loaded = loadShellResources({
      isPackaged: true,
      resourcesPath: value.packaged,
      files: value.files,
    });
    expect(loaded.profile).toEqual(value.profile);
    expect(loaded.manifestPath).toBe(path.join(value.packaged, 'manifest.json'));
    expect(loaded.provisioningPath).toBe(path.join(value.packaged, 'provisioning.json'));
  });

  it('rejects package identity or profile hash tampering', () => {
    const identity = fixture();
    const identityManifest = JSON.parse(
      fs.readFileSync(path.join(identity.packaged, 'manifest.json'), 'utf8')
    );
    identityManifest.product.id = 'other';
    fs.writeFileSync(
      path.join(identity.packaged, 'manifest.json'),
      JSON.stringify(identityManifest)
    );
    expect(() =>
      loadShellResources({
        isPackaged: true,
        resourcesPath: identity.packaged,
        files: identity.files,
      })
    ).toThrow('identity');

    const hash = fixture();
    const hashProfile = JSON.parse(
      fs.readFileSync(path.join(hash.packaged, 'profile.json'), 'utf8')
    );
    hashProfile.update.channel = 'tampered';
    fs.writeFileSync(path.join(hash.packaged, 'profile.json'), JSON.stringify(hashProfile));
    expect(() =>
      loadShellResources({ isPackaged: true, resourcesPath: hash.packaged, files: hash.files })
    ).toThrow('profile hash');
  });
});
