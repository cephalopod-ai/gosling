import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  ArtifactWorkbenchProvider,
  useArtifactWorkbench,
} from '../../contexts/ArtifactWorkbenchContext';
import { useArtifactRouter } from '../../contexts/ArtifactRouterContext';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { ArtifactPane } from './ArtifactPane';

vi.mock('../../contexts/ArtifactRouterContext', () => ({ useArtifactRouter: vi.fn() }));

describe('ArtifactPane', () => {
  const saveArtifact = vi.fn();

  function Harness() {
    const { openContent } = useArtifactWorkbench();
    return (
      <>
        <button
          type="button"
          onClick={() =>
            openContent({
              title: 'hero.png',
              content: 'AAEC',
              encoding: 'base64',
              mimeType: 'image/png',
              workspaceId: 'workspace-1',
            })
          }
        >
          Open image
        </button>
        <ArtifactPane />
      </>
    );
  }

  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    saveArtifact.mockResolvedValue({ canceled: false, filePath: '/outputs/images/hero.png' });
    vi.mocked(useArtifactRouter).mockReturnValue({
      saveArtifact,
      setVisibleSessionWorkspaceId: vi.fn(),
    });
  });

  it('saves a full transient artifact through its originating workspace', async () => {
    render(
      <IntlTestWrapper>
        <ArtifactWorkbenchProvider>
          <Harness />
        </ArtifactWorkbenchProvider>
      </IntlTestWrapper>
    );

    fireEvent.click(screen.getByRole('button', { name: 'Open image' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Save a copy' }));

    await waitFor(() =>
      expect(saveArtifact).toHaveBeenCalledWith({
        workspaceId: 'workspace-1',
        mimeType: 'image/png',
        suggestedName: 'hero.png',
        title: 'Save a copy',
        source: { type: 'content', content: 'AAEC', encoding: 'base64' },
      })
    );
  });
});
