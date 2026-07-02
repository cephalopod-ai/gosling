export type AcpFeatureCapabilities = Record<string, never>;

export async function getAcpFeatureCapabilities(): Promise<AcpFeatureCapabilities> {
  return {};
}
