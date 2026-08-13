import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import type { ResolvedShellProductProfile, ShellBuildManifest } from './profile';

export interface ShellResourceFiles {
  profileFileName: string;
  manifestFileName: string;
  provisioningFileName: string;
  developmentProfilePath: string;
  developmentManifestPath: string;
  developmentProvisioningPath: string;
}

export interface LoadedShellResources {
  profile: ResolvedShellProductProfile;
  manifest: ShellBuildManifest;
  profilePath: string;
  manifestPath: string;
  provisioningPath: string;
}

function readJson<T>(file: string): T {
  return JSON.parse(fs.readFileSync(file, 'utf8')) as T;
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(',')}]`;
  }
  if (value && typeof value === 'object') {
    return `{${Object.entries(value)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([key, child]) => `${JSON.stringify(key)}:${canonicalJson(child)}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function assertShellResourceIntegrity(
  profile: ResolvedShellProductProfile,
  manifest: ShellBuildManifest
): void {
  if (
    profile.schemaVersion !== 1 ||
    manifest.schemaVersion !== 1 ||
    manifest.profileSchemaVersion !== 1
  ) {
    throw new Error('shell package uses an unsupported profile or manifest schema');
  }
  const identityFields = [
    'id',
    'displayName',
    'version',
    'runtimeNamespace',
    'protocolScheme',
    'executableName',
    'macosBundleId',
    'windowsAppId',
    'linuxPackageName',
    'flatpakId',
  ] as const;
  for (const field of identityFields) {
    if (
      typeof profile.product?.[field] !== 'string' ||
      !profile.product[field] ||
      profile.product[field] !== manifest.product?.[field]
    ) {
      throw new Error('shell package identity is invalid or inconsistent');
    }
  }
  const profileHash = crypto.createHash('sha256').update(canonicalJson(profile)).digest('hex');
  if (profileHash !== manifest.profileHash) {
    throw new Error('shell package profile hash does not match its manifest');
  }
  if (
    manifest.compatibility.goslingVersion !== profile.compatibility.goslingVersion ||
    manifest.compatibility.provisioningSchemaVersion !==
      profile.compatibility.provisioningSchemaVersion ||
    manifest.compatibility.handoffSchemaVersion !== profile.compatibility.handoffSchemaVersion ||
    JSON.stringify([...manifest.compatibility.requiredMethods].sort()) !==
      JSON.stringify([...profile.compatibility.requiredMethods].sort())
  ) {
    throw new Error('shell package compatibility metadata is inconsistent');
  }
}

export function loadShellResources(input: {
  isPackaged: boolean;
  resourcesPath: string;
  files: ShellResourceFiles;
}): LoadedShellResources {
  const profilePath = input.isPackaged
    ? path.join(input.resourcesPath, input.files.profileFileName)
    : input.files.developmentProfilePath;
  const manifestPath = input.isPackaged
    ? path.join(input.resourcesPath, input.files.manifestFileName)
    : input.files.developmentManifestPath;
  const provisioningPath = input.isPackaged
    ? path.join(input.resourcesPath, input.files.provisioningFileName)
    : input.files.developmentProvisioningPath;
  const profile = readJson<ResolvedShellProductProfile>(profilePath);
  const manifest = readJson<ShellBuildManifest>(manifestPath);
  assertShellResourceIntegrity(profile, manifest);
  return {
    profile,
    manifest,
    profilePath,
    manifestPath,
    provisioningPath,
  };
}
