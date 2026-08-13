import type { BrowserWindowConstructorOptions } from 'electron';
import path from 'node:path';
import { startGoslingServe, type GoslingServeResult, type ShellHostProfile } from './goslingServe';

export interface MinimalShellHostOptions {
  profile: ShellHostProfile;
  serverSecret: string;
  workingDir?: string;
  diagnosticsDir?: string;
  processRegistryPath?: string;
  isPackaged?: boolean;
  resourcesPath?: string;
  preloadPath?: string;
  sessionPartition?: string;
}

export interface MinimalShellHostRuntime {
  backend: GoslingServeResult;
  windowOptions: BrowserWindowConstructorOptions;
}

export function createMinimalShellWindowOptions(
  options: Pick<MinimalShellHostOptions, 'profile' | 'preloadPath' | 'sessionPartition'>
): BrowserWindowConstructorOptions {
  return {
    width: 1180,
    height: 780,
    minWidth: 760,
    minHeight: 520,
    show: false,
    title: options.profile.displayName,
    webPreferences: {
      preload: options.preloadPath ?? path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      ...(options.sessionPartition ? { partition: options.sessionPartition } : {}),
    },
  };
}

export const createMinimalShellHost = async (
  options: MinimalShellHostOptions
): Promise<MinimalShellHostRuntime> => {
  const backend = await startGoslingServe({
    dir: options.workingDir,
    serverSecret: options.serverSecret,
    shell: options.profile,
    diagnosticsDir: options.diagnosticsDir,
    processRegistryPath: options.processRegistryPath,
    isPackaged: options.isPackaged,
    resourcesPath: options.resourcesPath,
  });

  return {
    backend,
    windowOptions: createMinimalShellWindowOptions(options),
  };
};
