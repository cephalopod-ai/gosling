export type ArtifactKind =
  | 'csv'
  | 'graphml'
  | 'html'
  | 'image'
  | 'json'
  | 'jsonl'
  | 'markdown'
  | 'pdf'
  | 'svg'
  | 'text'
  | 'unknown';

export type ArtifactSource =
  | {
      type: 'file';
      path: string;
      baseDirectory?: string;
    }
  | {
      type: 'content';
      content: string;
      encoding: 'base64' | 'utf8';
      mimeType: string;
    };

export interface ArtifactTab {
  id: string;
  kind: ArtifactKind;
  source: ArtifactSource;
  title: string;
}
