import type { Session } from 'electron';
import { describe, expect, it, vi } from 'vitest';
import type { ArtifactRoutingConfig } from '../types/artifactRouter';
import {
  availableDownloadPath,
  installArtifactDownloadRouter,
  routedDownloadPath,
} from './artifactDownloads';

const config: ArtifactRoutingConfig = {
  workspaceId: 'workspace-1',
  workspaceName: 'Campaign',
  outputs: [
    {
      id: 'default',
      isDefault: true,
      path: '/outputs/default',
      productTypes: ['document'],
    },
    {
      id: 'images',
      isDefault: false,
      path: '/outputs/images',
      productTypes: ['image'],
    },
  ],
};

describe('artifactDownloads', () => {
  it('routes native downloads by product type', () => {
    expect(routedDownloadPath(config, 'hero.png', 'image/png', () => false)).toBe(
      '/outputs/images/hero.png'
    );
    expect(routedDownloadPath(config, 'notes.unknown', undefined, () => false)).toBe(
      '/outputs/default/notes.unknown'
    );
  });

  it('uses collision-safe names without overwriting existing artifacts', () => {
    const occupied = new Set(['/outputs/slides/deck.pptx', '/outputs/slides/deck (1).pptx']);
    expect(
      availableDownloadPath('/outputs/slides', 'deck.pptx', (name) => occupied.has(name))
    ).toBe('/outputs/slides/deck (2).pptx');
  });

  it('cannot escape the routed directory through a download filename', () => {
    expect(routedDownloadPath(config, '../../secrets.txt', undefined, () => false)).toBe(
      '/outputs/default/secrets.txt'
    );
  });

  it('reserves simultaneous names and reports downloads that cannot be routed', () => {
    type Listener = (event: unknown, item: DownloadItemStub, webContents: { id: number }) => void;
    interface DownloadItemStub {
      getFilename(): string;
      getMimeType(): string;
      once: ReturnType<typeof vi.fn>;
      setSavePath(destination: string): void;
    }

    let listener: Listener | undefined;
    const electronSession = {
      on: vi.fn((_event: string, callback: Listener) => {
        listener = callback;
      }),
    } as unknown as Session;
    const onUnrouted = vi.fn();
    installArtifactDownloadRouter(
      electronSession,
      (id) => (id === 1 ? config : undefined),
      onUnrouted
    );

    const destinations: string[] = [];
    const item = (): DownloadItemStub => ({
      getFilename: () => 'brief.pdf',
      getMimeType: () => 'application/pdf',
      setSavePath: (destination) => destinations.push(destination),
      once: vi.fn(),
    });
    listener?.({}, item(), { id: 1 });
    listener?.({}, item(), { id: 1 });
    listener?.({}, item(), { id: 2 });

    expect(destinations).toEqual(['/outputs/default/brief.pdf', '/outputs/default/brief (1).pdf']);
    expect(onUnrouted).toHaveBeenCalledWith(2, 'brief.pdf');
  });
});
