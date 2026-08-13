const path = require('node:path');
const { resolveProfile, writeBuildResolution } = require('./shell-profile');

const SHELL_PROFILE_ENV = 'GOSLING_SHELL_PROFILE';
const RETIRED_IDENTITY_ENV = [
  'GOSLING_SHELL_PRODUCT_NAME',
  'GOSLING_SHELL_PROTOCOL_SCHEME',
  'GOSLING_SHELL_PACKAGE_ID',
];

function fail(message) {
  throw new Error(message);
}

function targetFor(platform = process.platform, architecture = process.arch) {
  const platformName = { darwin: 'macos', win32: 'windows', linux: 'linux' }[platform];
  const architectureName = { arm64: 'arm64', x64: 'x64' }[architecture];
  if (!platformName || !architectureName)
    fail(`unsupported Forge target ${platform}/${architecture}`);
  const target = `${platformName}-${architectureName}`;
  if (target === 'windows-arm64' || target === 'linux-arm64') {
    fail(`unsupported Forge target ${platform}/${architecture}`);
  }
  return target;
}

function defaultProjection(env = process.env) {
  return {
    shell: false,
    productName: 'Gosling',
    executableName: 'Gosling',
    version: undefined,
    protocolScheme: 'gosling',
    macosBundleId: undefined,
    windowsAppId: undefined,
    linuxPackageName: 'Gosling',
    flatpakId: 'io.github.repo_makeover.Gosling',
    iconBase: 'src/images/icon',
    iconIco: 'src/images/icon.ico',
    iconPng: 'src/images/icon.png',
    iconFlatpak512: 'src/images/icon-512.png',
    iconSvg: 'src/images/icon.svg',
    extraResource: ['src/bin', 'src/images', 'src/app-update.yml'],
    update: {
      enabled: true,
      owner: env.GITHUB_OWNER || 'repo-makeover',
      repository: env.GITHUB_REPO || 'gosling',
    },
    shellResources: undefined,
    resolved: undefined,
  };
}

function assertNoRetiredIdentityOverrides(env) {
  const overridden = RETIRED_IDENTITY_ENV.find((name) => env[name] !== undefined);
  if (overridden) fail(`${overridden} is retired; select one source-controlled shell profile`);
}

function profileProjection(profileFile, platform, architecture) {
  const resolved = resolveProfile(profileFile);
  const target = targetFor(platform, architecture);
  const targetAssets = resolved.assetsByTarget[target];
  if (!targetAssets) fail(`profile.assets.requiredTargets does not include ${target}`);
  const profile = resolved.profile;
  const buildResolution = writeBuildResolution(resolved, target);
  const relativeAsset = (file) =>
    path.relative(path.join(resolved.repositoryRoot, 'ui', 'desktop'), file);
  return {
    shell: true,
    productName: profile.product.displayName,
    executableName: profile.product.executableName,
    version: profile.product.version,
    protocolScheme: profile.product.protocolScheme,
    macosBundleId: profile.product.macosBundleId,
    windowsAppId: profile.product.windowsAppId,
    linuxPackageName: profile.product.linuxPackageName,
    flatpakId: profile.product.flatpakId,
    iconBase:
      target.startsWith('macos-') && targetAssets.icon
        ? relativeAsset(targetAssets.icon).replace(/\.icns$/, '')
        : undefined,
    iconIco: target === 'windows-x64' ? relativeAsset(targetAssets.icon) : undefined,
    iconPng: targetAssets.iconPng ? relativeAsset(targetAssets.iconPng) : undefined,
    iconFlatpak512: targetAssets.iconPng ? relativeAsset(targetAssets.iconPng) : undefined,
    iconSvg: targetAssets.iconSvg ? relativeAsset(targetAssets.iconSvg) : undefined,
    extraResource: [
      'src/bin',
      relativeAsset(buildResolution.profileOutput),
      relativeAsset(buildResolution.manifestOutput),
      relativeAsset(resolved.provisioningPath),
      ...Object.values(targetAssets).map(relativeAsset),
    ],
    shellResources: {
      profileFileName: path.basename(buildResolution.profileOutput),
      manifestFileName: path.basename(buildResolution.manifestOutput),
      provisioningFileName: path.basename(resolved.provisioningPath),
      developmentProfilePath: buildResolution.profileOutput,
      developmentManifestPath: buildResolution.manifestOutput,
      developmentProvisioningPath: resolved.provisioningPath,
    },
    update: {
      enabled: profile.update.enabled,
      owner: profile.update.owner,
      repository: profile.update.repository,
    },
    resolved,
  };
}

function resolveForgeProjection(
  env = process.env,
  platform = process.platform,
  architecture = env.ELECTRON_ARCH || process.arch
) {
  assertNoRetiredIdentityOverrides(env);
  const profileFile = env[SHELL_PROFILE_ENV];
  if (!profileFile) return defaultProjection(env);
  return profileProjection(profileFile, platform, architecture);
}

module.exports = {
  RETIRED_IDENTITY_ENV,
  SHELL_PROFILE_ENV,
  defaultProjection,
  profileProjection,
  resolveForgeProjection,
  targetFor,
};
