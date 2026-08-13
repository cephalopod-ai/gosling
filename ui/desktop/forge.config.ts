const { FusesPlugin } = require('@electron-forge/plugin-fuses');
const { FuseV1Options, FuseVersion } = require('@electron/fuses');
const { resolveForgeProjection } = require('./scripts/shell-forge-profile');

const isLinuxVulkanBuild = process.env.GOSLING_DESKTOP_LINUX_VARIANT === 'vulkan';
const product = resolveForgeProjection();
const signingAllowed = !product.shell || product.resolved.profile.distribution.publishable;
const viteEntries = product.shell
  ? {
      build: [
        {
          entry: 'src/shell/main.ts',
          config: 'vite.shell.main.config.mts',
        },
        {
          entry: 'src/shell/preload.ts',
          config: 'vite.shell.preload.config.mts',
        },
      ],
      renderer: [
        {
          name: 'shell_window',
          config: 'vite.shell.renderer.config.mts',
        },
      ],
    }
  : {
      build: [
        {
          entry: 'src/main.ts',
          config: 'vite.main.config.mts',
        },
        {
          entry: 'src/preload.ts',
          config: 'vite.preload.config.mts',
        },
      ],
      renderer: [
        {
          name: 'main_window',
          config: 'vite.renderer.config.mts',
        },
      ],
    };

let cfg = {
  name: product.productName,
  executableName: product.executableName,
  ...(product.version ? { appVersion: product.version } : {}),
  ...(product.macosBundleId ? { appBundleId: product.macosBundleId } : {}),
  asar: true,
  extraResource: product.extraResource,
  icon: product.iconBase,
  // Windows specific configuration
  win32: {
    icon: product.iconIco,
    ...(signingAllowed
      ? {
          certificateFile: process.env.WINDOWS_CERTIFICATE_FILE,
          signingRole: process.env.WINDOW_SIGNING_ROLE,
          rfc3161TimeStampServer: 'http://timestamp.digicert.com',
          signWithParams: '/fd sha256 /tr http://timestamp.digicert.com /td sha256',
        }
      : {}),
  },
  ...(product.windowsAppId
    ? {
        win32metadata: {
          ProductName: product.productName,
          InternalName: product.windowsAppId,
          OriginalFilename: `${product.executableName}.exe`,
        },
      }
    : {}),
  // Protocol registration
  protocols: [
    {
      name: `${product.productName}Protocol`,
      schemes: [product.protocolScheme],
    },
  ],
  // macOS Info.plist extensions for drag-and-drop support
  extendInfo: {
    // Document types for drag-and-drop support onto dock icon
    CFBundleDocumentTypes: [
      {
        CFBundleTypeName: 'Folders',
        CFBundleTypeRole: 'Viewer',
        LSHandlerRank: 'Alternate',
        LSItemContentTypes: ['public.directory', 'public.folder'],
      },
    ],
    // Usage descriptions for macOS TCC (Transparency, Consent, and Control)
    NSCalendarsUsageDescription:
      'Gosling needs access to your calendars to help manage and query calendar events.',
    NSRemindersUsageDescription:
      'Gosling needs access to your reminders to help manage and query reminders.',
  },
};

// macOS code signing and notarization via Electron Forge
// Activated when APPLE_TEAM_ID is set (CI signing builds)
if (process.env.APPLE_TEAM_ID && signingAllowed) {
  cfg.osxSign = {
    keychain: process.env.KEYCHAIN_PATH || undefined,
    entitlements: 'entitlements.plist',
    'entitlements-inherit': 'entitlements.plist',
  };
  cfg.osxNotarize = {
    appleId: process.env.APPLE_ID,
    appleIdPassword: process.env.APPLE_ID_PASSWORD,
    teamId: process.env.APPLE_TEAM_ID,
  };
}

module.exports = {
  packagerConfig: cfg,
  rebuildConfig: {},
  publishers: product.update.enabled
    ? [
        {
          name: '@electron-forge/publisher-github',
          config: {
            repository: {
              owner: product.update.owner,
              name: product.update.repository,
            },
            prerelease: false,
            draft: true,
          },
        },
      ]
    : [],
  makers: [
    {
      name: '@electron-forge/maker-zip',
      platforms: ['darwin', 'win32', 'linux'],
      config: {
        arch: process.env.ELECTRON_ARCH === 'x64' ? ['x64'] : ['arm64'],
        options: {
          icon: product.iconIco,
        },
      },
    },
    {
      name: '@electron-forge/maker-deb',
      config: {
        name: product.linuxPackageName,
        bin: product.executableName,
        maintainer: 'repo-makeover',
        homepage: 'https://gosling-docs.ai/',
        categories: ['Development'],
        desktopTemplate: './forge.deb.desktop',
        options: {
          icon: product.iconPng,
          prefix: '/opt',
          ...(isLinuxVulkanBuild ? { depends: ['libvulkan1'] } : {}),
        },
      },
    },
    {
      name: '@electron-forge/maker-rpm',
      config: {
        name: product.linuxPackageName,
        bin: product.executableName,
        maintainer: 'repo-makeover',
        homepage: 'https://gosling-docs.ai/',
        categories: ['Development'],
        desktopTemplate: './forge.rpm.desktop',
        options: {
          icon: product.iconPng,
          prefix: '/opt',
          ...(isLinuxVulkanBuild ? { requires: ['vulkan-loader'] } : {}),
        },
      },
    },
    {
      name: '@electron-forge/maker-flatpak',
      config: {
        options: {
          id: product.flatpakId,
          categories: ['Development'],
          icon: {
            scalable: product.iconSvg,
            '512x512': product.iconFlatpak512,
          },
          homepage: 'https://gosling-docs.ai/',
          runtimeVersion: '25.08',
          baseVersion: '25.08',
          bin: product.executableName,
          modules: [
            {
              name: 'libbz2-shim',
              buildsystem: 'simple',
              'build-commands': [
                // Create the lib directory in the app bundle
                'mkdir -p /app/lib',
                // Point to the actual library in the 25.08 runtime
                // We use a wildcard to handle multi-arch paths (x86_64-linux-gnu, etc)
                'ln -s $(find /usr/lib -name "libbz2.so.1" | head -n 1) /app/lib/libbz2.so.1.0',
              ],
            },
          ],
          finishArgs: [
            '--share=ipc',
            '--socket=x11',
            '--socket=wayland',
            '--device=dri',
            '--share=network',
            '--filesystem=home',
            '--talk-name=org.freedesktop.Notifications',
            '--socket=session-bus',
            '--socket=system-bus',
            // This ensures the app looks in our shim folder first
            '--env=LD_LIBRARY_PATH=/app/lib',
          ],
        },
      },
    },
  ],
  plugins: [
    {
      name: '@electron-forge/plugin-vite',
      config: viteEntries,
    },
    // Fuses are used to enable/disable various Electron functionality
    // at package time, before code signing the application
    new FusesPlugin({
      version: FuseVersion.V1,
      [FuseV1Options.RunAsNode]: false,
      [FuseV1Options.EnableCookieEncryption]: true,
      [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,
      [FuseV1Options.EnableNodeCliInspectArguments]: false,
      [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,
      [FuseV1Options.OnlyLoadAppFromAsar]: true,
    }),
  ],
};
