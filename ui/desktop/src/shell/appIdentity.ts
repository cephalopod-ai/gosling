import path from 'node:path';
import type { ShellProductIdentity } from './profile';

export interface ShellAppPaths {
  userData: string;
  sessionData: string;
  cache: string;
  temp: string;
  logs: string;
  diagnostics: string;
  processRegistry: string;
}

export interface ShellAppIdentity {
  name: string;
  protocolScheme: string;
  sessionPartition: string;
  paths: ShellAppPaths;
}

export interface ShellAppIdentityAdapter {
  getPath(name: 'appData' | 'cache' | 'temp'): string;
  setName(name: string): void;
  setPath(name: 'userData' | 'sessionData' | 'cache' | 'temp' | 'logs', value: string): void;
}

export function deriveShellAppIdentity(
  product: ShellProductIdentity,
  roots: { appData: string; cache: string; temp: string }
): ShellAppIdentity {
  const userData = path.join(roots.appData, product.id);
  const cache = path.join(roots.cache, product.id);
  const temp = path.join(roots.temp, product.id);
  return {
    name: product.displayName,
    protocolScheme: product.protocolScheme,
    sessionPartition: `persist:gosling-shell-${product.id}`,
    paths: {
      userData,
      sessionData: path.join(userData, 'session'),
      cache,
      temp,
      logs: path.join(userData, 'logs'),
      diagnostics: path.join(userData, 'diagnostics'),
      processRegistry: path.join(userData, 'backend-processes.json'),
    },
  };
}

export function applyShellAppIdentity(
  app: ShellAppIdentityAdapter,
  product: ShellProductIdentity
): ShellAppIdentity {
  const identity = deriveShellAppIdentity(product, {
    appData: app.getPath('appData'),
    cache: app.getPath('cache'),
    temp: app.getPath('temp'),
  });
  app.setName(identity.name);
  app.setPath('userData', identity.paths.userData);
  app.setPath('sessionData', identity.paths.sessionData);
  app.setPath('cache', identity.paths.cache);
  app.setPath('temp', identity.paths.temp);
  app.setPath('logs', identity.paths.logs);
  return identity;
}
