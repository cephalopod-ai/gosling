import { beforeEach, describe, expect, it, vi } from 'vitest';
import { acpExportSession, type SessionListItem } from '../acp/sessions';
import { saveSessionArtifactExport } from './sessionArtifactExport';

vi.mock('../acp/sessions', () => ({ acpExportSession: vi.fn() }));

describe('saveSessionArtifactExport', () => {
  beforeEach(() => vi.clearAllMocks());

  it('pins session exports to the session workspace instead of the active workspace', async () => {
    vi.mocked(acpExportSession).mockResolvedValue('{"session":"one"}');
    const saveArtifact = vi.fn().mockResolvedValue({ canceled: false });
    const session = {
      id: 'session-1',
      name: 'Quarterly / Review',
      workspaceId: 'workspace-pinned',
    } as SessionListItem;

    await saveSessionArtifactExport(session, saveArtifact);

    expect(saveArtifact).toHaveBeenCalledWith(
      expect.objectContaining({
        workspaceId: 'workspace-pinned',
        productType: 'export',
        suggestedName: 'Quarterly / Review.json',
        source: { type: 'content', content: '{"session":"one"}', encoding: 'utf8' },
      })
    );
  });
});
