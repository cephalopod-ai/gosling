/** The main process pins this namespace for the window's lifetime, even if settings change. */
export function getBackendStorageId(): string | undefined {
  const backendId = window.appConfig?.get('GOSLING_BACKEND_ID');
  return typeof backendId === 'string' && backendId ? backendId : undefined;
}

export function backendStorageKey(key: string): string {
  const backendId = getBackendStorageId();
  return backendId ? `${key}:${backendId}` : key;
}
