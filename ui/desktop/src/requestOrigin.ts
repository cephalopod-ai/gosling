export function getOverrideOriginForRequest(
  requestUrl: string,
  devServerUrl?: string | null
): string | null {
  if (!devServerUrl) {
    return null;
  }

  try {
    const devOrigin = new URL(devServerUrl).origin;
    const requestOrigin = new URL(requestUrl).origin;
    return requestOrigin === devOrigin ? devOrigin : null;
  } catch {
    return null;
  }
}
