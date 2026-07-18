import type { ArtifactRoutingConfig } from '../types/artifactRouter';

type ArtifactRoutingConfigValidator = (
  config: ArtifactRoutingConfig
) => Promise<ArtifactRoutingConfig | null>;

export class ArtifactRoutingRegistry {
  private readonly configs = new Map<number, ArtifactRoutingConfig>();
  private readonly revisions = new Map<number, number>();

  get(webContentsId: number): ArtifactRoutingConfig | undefined {
    return this.configs.get(webContentsId);
  }

  clear(webContentsId: number): void {
    this.nextRevision(webContentsId);
    this.configs.delete(webContentsId);
  }

  async update(
    webContentsId: number,
    config: ArtifactRoutingConfig | null,
    validate: ArtifactRoutingConfigValidator
  ): Promise<boolean> {
    const revision = this.nextRevision(webContentsId);
    if (!config) {
      this.configs.delete(webContentsId);
      return true;
    }

    const validated = await validate(config);
    if (this.revisions.get(webContentsId) !== revision) {
      return false;
    }

    if (!validated) {
      this.configs.delete(webContentsId);
      return false;
    }

    this.configs.set(webContentsId, validated);
    return true;
  }

  private nextRevision(webContentsId: number): number {
    const revision = (this.revisions.get(webContentsId) ?? 0) + 1;
    this.revisions.set(webContentsId, revision);
    return revision;
  }
}
