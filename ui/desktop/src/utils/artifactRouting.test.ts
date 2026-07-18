import { describe, expect, it } from 'vitest';
import type { Workspace } from '@repo-makeover/gosling-sdk';
import {
  inferArtifactProductType,
  joinArtifactPath,
  resolveWorkspaceArtifact,
  safeArtifactFileName,
  selectArtifactOutput,
  suggestedArtifactFileName,
} from './artifactRouting';

const workspace = {
  id: 'workspace-1',
  schemaVersion: 1,
  name: 'Campaign',
  workingFolder: '/projects/campaign',
  folders: [],
  productOutputFolders: [
    {
      id: 'documents',
      label: 'Documents',
      path: '/outputs/documents',
      productTypes: ['document'],
      isDefault: true,
      createIfMissing: false,
    },
    {
      id: 'slides',
      label: 'Presentations',
      path: '/outputs/slides',
      productTypes: ['presentation'],
      isDefault: false,
      createIfMissing: false,
    },
    {
      id: 'images',
      label: 'Images',
      path: 'C:\\Outputs\\Images',
      productTypes: ['image'],
      isDefault: false,
      createIfMissing: false,
    },
  ],
  credentialBindings: [],
  createdAt: '2026-07-18T00:00:00Z',
  updatedAt: '2026-07-18T00:00:00Z',
  lastOpenedAt: '2026-07-18T00:00:00Z',
} satisfies Workspace;

describe('artifactRouting', () => {
  it('classifies office, image, data, and explicit export artifacts', () => {
    expect(inferArtifactProductType({ suggestedName: 'brief.pptx' })).toBe('presentation');
    expect(inferArtifactProductType({ suggestedName: 'photo.bin', mimeType: 'image/png' })).toBe(
      'image'
    );
    expect(inferArtifactProductType({ suggestedName: 'rows.jsonl' })).toBe('data');
    expect(inferArtifactProductType({ suggestedName: 'session.json', productType: 'export' })).toBe(
      'export'
    );
    expect(inferArtifactProductType({ suggestedName: 'deck.pptx', mimeType: 'text/html' })).toBe(
      'presentation'
    );
  });

  it('selects a matching output before the default and falls back deterministically', () => {
    expect(selectArtifactOutput(workspace.productOutputFolders, 'presentation')?.id).toBe('slides');
    expect(selectArtifactOutput(workspace.productOutputFolders, 'video')?.id).toBe('documents');
  });

  it('builds platform-aware default paths and strips path injection from names', () => {
    const image = resolveWorkspaceArtifact(workspace, {
      suggestedName: '../draft:hero?.png',
      mimeType: 'image/png',
    });
    expect(image.defaultPath).toBe('C:\\Outputs\\Images\\draft-hero-.png');
    expect(joinArtifactPath('/outputs/docs/', 'nested/report.pdf')).toBe(
      '/outputs/docs/report.pdf'
    );
    expect(safeArtifactFileName('..')).toBe('artifact');
    expect(safeArtifactFileName('CON.txt')).toBe('_CON.txt');
    expect(suggestedArtifactFileName('tool output', 'image/png')).toBe('tool output.png');
  });

  it('caps portable filenames by UTF-8 bytes without splitting the extension', () => {
    const fileName = safeArtifactFileName(`${'🐦'.repeat(80)}.png`);

    expect(new TextEncoder().encode(fileName).length).toBeLessThanOrEqual(180);
    expect(fileName.endsWith('.png')).toBe(true);
    expect(fileName.includes('�')).toBe(false);
  });
});
