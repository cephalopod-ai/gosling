import type { ProductType } from '@repo-makeover/gosling-sdk';

export interface ArtifactRoutingOutput {
  id: string;
  isDefault: boolean;
  path: string;
  productTypes: ProductType[];
}

export interface ArtifactRoutingConfig {
  outputs: ArtifactRoutingOutput[];
  workspaceId: string;
  workspaceName: string;
}

export type ArtifactSaveSource =
  | {
      content: string;
      encoding: 'base64' | 'utf8';
      type: 'content';
    }
  | {
      baseDirectory?: string;
      path: string;
      type: 'file';
    };

export interface ArtifactSaveRequest {
  defaultPath: string;
  filters?: Array<{ name: string; extensions: string[] }>;
  source: ArtifactSaveSource;
  title?: string;
}

export interface ArtifactSaveResponse {
  canceled: boolean;
  filePath?: string;
}

export interface RoutedArtifactSaveInput {
  filters?: Array<{ name: string; extensions: string[] }>;
  mimeType?: string;
  productType?: ProductType;
  source: ArtifactSaveSource;
  suggestedName: string;
  title?: string;
  workspaceId?: string | null;
}

export interface RoutedArtifactSaveResult extends ArtifactSaveResponse {
  outputFolderId?: string;
  productType?: ProductType;
  workspaceId?: string;
}
