import type { Session } from 'electron';
import { describe, expect, it, vi } from 'vitest';
import type { ArtifactRoutingConfig } from '../types/artifactRouter';
import {
  availableDownloadPath,
  installArtifactDownloadRouter,
  routedDownloadPath,
  routesVisibleChat,
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
    type Listener = (
      event: unknown,
      item: DownloadItemStub,
      webContents: { id: number; getURL(): string }
    ) => void;
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
    const hub = (id: number) => ({ id, getURL: () => 'file:///app/index.html#/' });
    listener?.({}, item(), hub(1));
    listener?.({}, item(), hub(1));
    listener?.({}, item(), hub(2));

    expect(destinations).toEqual(['/outputs/default/brief.pdf', '/outputs/default/brief (1).pdf']);
    expect(onUnrouted).toHaveBeenCalledWith(2, 'brief.pdf');
  });

  // Right after a chat switch the stored route still belongs to the previous chat until the
  // renderer has rendered the new one and republished (GSL-PT-20260927-G221).
  it('applies a route only while the window still shows the chat it was published for', () => {
    const chatA = { ...config, sessionId: 'chat-a' };
    const url = (route: string) => `file:///app/index.html#${route}`;

    expect(routesVisibleChat(chatA, url('/pair?resumeSessionId=chat-a'))).toBe(true);
    expect(routesVisibleChat(chatA, url('/pair?resumeSessionId=chat-b'))).toBe(false);
    expect(routesVisibleChat(chatA, url('/settings'))).toBe(false);
    expect(routesVisibleChat(config, url('/'))).toBe(true);
    expect(routesVisibleChat(config, url('/pair?resumeSessionId=chat-b'))).toBe(false);
  });

  it('never saves a download into the previous chat when the window already shows another', () => {
    type Listener = (
      event: unknown,
      item: { getFilename(): string; getMimeType(): string; setSavePath(path: string): void },
      webContents: { id: number; getURL(): string }
    ) => void;
    let listener: Listener | undefined;
    const electronSession = {
      on: vi.fn((_event: string, callback: Listener) => {
        listener = callback;
      }),
    } as unknown as Session;
    const onUnrouted = vi.fn();
    installArtifactDownloadRouter(
      electronSession,
      () => ({ ...config, sessionId: 'chat-a' }),
      onUnrouted
    );
    const setSavePath = vi.fn();

    listener?.(
      {},
      { getFilename: () => 'notes.md', getMimeType: () => 'text/markdown', setSavePath },
      { id: 1, getURL: () => 'file:///app/index.html#/pair?resumeSessionId=chat-b' }
    );

    expect(setSavePath).not.toHaveBeenCalled();
    expect(onUnrouted).toHaveBeenCalledWith(1, 'notes.md');
  });
});
