import type {
  ResolvedShellProductProfile,
  ShellBuildManifest,
  ShellProductIdentity,
} from './profile';

export type ShellCompatibilityCode =
  | 'PROFILE_SCHEMA_UNSUPPORTED'
  | 'IDENTITY_MISMATCH'
  | 'CORE_MISMATCH'
  | 'PROVISIONING_SCHEMA_UNSUPPORTED'
  | 'METHOD_UNAVAILABLE'
  | 'RUNTIME_NAMESPACE_MISMATCH'
  | 'ADAPTER_MISMATCH'
  | 'PROVISIONING_INVALID';

export type ShellRuntimeIdentity = Pick<ShellProductIdentity, 'id' | 'displayName' | 'version'>;

export interface ShellRuntimeMetadata {
  identity: ShellRuntimeIdentity;
  runtimeNamespace: string;
  coreVersion: string;
  availableMethods: string[];
  domainAdapter: {
    descriptorId: string;
    protocolVersion: string;
    actions: string[];
  } | null;
}

export interface ShellProvisioningPreflight {
  schemaVersion: number;
  identity: ShellRuntimeIdentity;
  runtimeNamespace: string;
  valid: boolean;
}

export interface ShellCompatibilityFailure {
  compatible: false;
  code: ShellCompatibilityCode;
  expected: unknown;
  actual: unknown;
}

export interface ShellCompatibilitySuccess {
  compatible: true;
}

export type ShellCompatibilityResult = ShellCompatibilityFailure | ShellCompatibilitySuccess;

export function checkShellMethods(
  requiredMethods: readonly string[],
  availableMethods: readonly string[]
): ShellCompatibilityResult {
  const available = new Set(availableMethods);
  const missing = requiredMethods.filter((method) => !available.has(method));
  if (missing.length > 0) {
    return {
      compatible: false,
      code: 'METHOD_UNAVAILABLE',
      expected: [...requiredMethods],
      actual: [...available].sort(),
    };
  }
  return { compatible: true };
}

function runtimeIdentity(identity: ShellRuntimeIdentity): ShellRuntimeIdentity {
  return {
    id: identity.id,
    displayName: identity.displayName,
    version: identity.version,
  };
}

function sameIdentity(expected: ShellRuntimeIdentity, actual: ShellRuntimeIdentity): boolean {
  return (
    expected.id === actual.id &&
    expected.displayName === actual.displayName &&
    expected.version === actual.version
  );
}

export function checkShellCompatibility(input: {
  profile: ResolvedShellProductProfile;
  manifest: ShellBuildManifest;
  runtime: ShellRuntimeMetadata;
  provisioning: ShellProvisioningPreflight;
}): ShellCompatibilityResult {
  const { profile, manifest, runtime, provisioning } = input;
  if (profile.schemaVersion !== 1 || manifest.profileSchemaVersion !== 1) {
    return {
      compatible: false,
      code: 'PROFILE_SCHEMA_UNSUPPORTED',
      expected: 1,
      actual: { profile: profile.schemaVersion, manifest: manifest.profileSchemaVersion },
    };
  }
  const expectedIdentity = runtimeIdentity(profile.product);
  const manifestIdentity = runtimeIdentity(manifest.product);
  if (
    !sameIdentity(expectedIdentity, manifestIdentity) ||
    !sameIdentity(expectedIdentity, runtime.identity) ||
    !sameIdentity(expectedIdentity, provisioning.identity)
  ) {
    return {
      compatible: false,
      code: 'IDENTITY_MISMATCH',
      expected: expectedIdentity,
      actual: {
        manifest: manifestIdentity,
        runtime: runtimeIdentity(runtime.identity),
        provisioning: runtimeIdentity(provisioning.identity),
      },
    };
  }
  if (
    profile.product.runtimeNamespace !== manifest.product.runtimeNamespace ||
    profile.product.runtimeNamespace !== runtime.runtimeNamespace ||
    profile.product.runtimeNamespace !== provisioning.runtimeNamespace
  ) {
    return {
      compatible: false,
      code: 'RUNTIME_NAMESPACE_MISMATCH',
      expected: profile.product.runtimeNamespace,
      actual: {
        manifest: manifest.product.runtimeNamespace,
        runtime: runtime.runtimeNamespace,
        provisioning: provisioning.runtimeNamespace,
      },
    };
  }
  if (
    profile.compatibility.goslingVersion !== manifest.compatibility.goslingVersion ||
    profile.compatibility.goslingVersion !== runtime.coreVersion ||
    !/^[0-9a-f]{40}$/.test(manifest.compatibility.goslingRevision)
  ) {
    return {
      compatible: false,
      code: 'CORE_MISMATCH',
      expected: {
        version: profile.compatibility.goslingVersion,
        revision: manifest.compatibility.goslingRevision,
      },
      actual: { version: runtime.coreVersion, revision: manifest.compatibility.goslingRevision },
    };
  }
  if (
    profile.compatibility.provisioningSchemaVersion !== provisioning.schemaVersion ||
    manifest.compatibility.provisioningSchemaVersion !== provisioning.schemaVersion
  ) {
    return {
      compatible: false,
      code: 'PROVISIONING_SCHEMA_UNSUPPORTED',
      expected: profile.compatibility.provisioningSchemaVersion,
      actual: provisioning.schemaVersion,
    };
  }
  const methods = checkShellMethods(
    profile.compatibility.requiredMethods,
    runtime.availableMethods
  );
  if (!methods.compatible) {
    return methods;
  }
  const expectedAdapter = manifest.consumer?.domainAdapter;
  if (expectedAdapter) {
    const actualAdapter = runtime.domainAdapter;
    if (
      !actualAdapter ||
      actualAdapter.descriptorId !== expectedAdapter.descriptorId ||
      actualAdapter.protocolVersion !== expectedAdapter.protocolVersion ||
      actualAdapter.actions.join('\u0000') !== expectedAdapter.actions.join('\u0000')
    ) {
      return {
        compatible: false,
        code: 'ADAPTER_MISMATCH',
        expected: expectedAdapter,
        actual: actualAdapter,
      };
    }
  }
  if (!provisioning.valid) {
    return {
      compatible: false,
      code: 'PROVISIONING_INVALID',
      expected: true,
      actual: false,
    };
  }
  return { compatible: true };
}
