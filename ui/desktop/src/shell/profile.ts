export type ShellTarget = 'macos-arm64' | 'macos-x64' | 'windows-x64' | 'linux-x64';

export interface ShellProductIdentity {
  id: string;
  displayName: string;
  version: string;
  runtimeNamespace: string;
  protocolScheme: string;
  executableName: string;
  macosBundleId: string;
  windowsAppId: string;
  linuxPackageName: string;
  flatpakId: string;
}

export interface ResolvedShellProductProfile {
  schemaVersion: 1;
  product: ShellProductIdentity;
  provisioningPath: string;
  compatibility: {
    goslingVersion: string;
    goslingRevision: 'current' | string;
    provisioningSchemaVersion: 1;
    handoffSchemaVersion: 1;
    requiredMethods: string[];
  };
  assets: {
    root: string;
    iconBase: string;
    requiredTargets: ShellTarget[];
  };
  update: {
    enabled: boolean;
    channel: string;
    owner?: string;
    repository?: string;
  };
  distribution: {
    publishable: boolean;
    artifactPrefix: string;
    releaseDestination?: string;
    signingPolicy: 'none' | 'required';
  };
}

export interface ShellBuildManifest {
  schemaVersion: 1;
  profileSchemaVersion: 1;
  profileHash: string;
  product: ShellProductIdentity;
  target: ShellTarget;
  platform: 'macos' | 'windows' | 'linux';
  architecture: 'arm64' | 'x64';
  sourceClean: boolean;
  compatibility: {
    goslingVersion: string;
    goslingRevision: string;
    provisioningSchemaVersion: 1;
    handoffSchemaVersion: 1;
    requiredMethods: string[];
  };
  consumer?: {
    consumerId: string;
    consumerHash: string;
    rendererHash: string;
    declaredCapabilities: string[];
    requiredAgentCapabilities: string[];
    requiredMethods: string[];
    domainAdapter?: {
      descriptorId: string;
      protocolVersion: string;
      actions: string[];
    };
  };
}
