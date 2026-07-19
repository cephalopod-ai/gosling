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
  const readArtifactFile = vi.fn();

  function Harness() {
    const { openContent, openFile } = useArtifactWorkbench();
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
        <button
          type="button"
          onClick={() => openFile('/outputs/report.md', '/outputs', 'workspace-1')}
        >
          Open file
        </button>
        <ArtifactPane />
      </>
    );
  }

  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    saveArtifact.mockResolvedValue({ canceled: false, filePath: '/outputs/images/hero.png' });
    readArtifactFile.mockReset();
    readArtifactFile.mockResolvedValue({
      content: '',
      encoding: 'utf8',
      error: 'Renderer file access denied for path outside approved roots',
      filePath: '/outputs/report.md',
      found: false,
      sizeBytes: 0,
      truncated: false,
    });
    Object.assign(window.electron, { readArtifactFile });
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

  it('keeps pane controls outside the native titlebar drag region', () => {
    render(
      <IntlTestWrapper>
        <ArtifactWorkbenchProvider>
          <Harness />
        </ArtifactWorkbenchProvider>
      </IntlTestWrapper>
    );

    expect(screen.getByTitle('Open file')).toHaveClass('no-drag');
    expect(screen.getByTitle('Close outputs pane')).toHaveClass('no-drag');
  });

  it('retries a transient route authorization failure before showing an error', async () => {
    readArtifactFile
      .mockResolvedValueOnce({
        content: '',
        encoding: 'utf8',
        error: 'Renderer file access denied for path outside approved roots',
        filePath: '/outputs/report.md',
        found: false,
        sizeBytes: 0,
        truncated: false,
      })
      .mockResolvedValueOnce({
        content: '# Report',
        encoding: 'utf8',
        error: null,
        filePath: '/outputs/report.md',
        found: true,
        sizeBytes: 9,
        truncated: false,
      });

    render(
      <IntlTestWrapper>
        <ArtifactWorkbenchProvider>
          <Harness />
        </ArtifactWorkbenchProvider>
      </IntlTestWrapper>
    );

    fireEvent.click(screen.getAllByRole('button', { name: 'Open file' })[0]);

    await waitFor(() => expect(readArtifactFile).toHaveBeenCalledTimes(2), { timeout: 1000 });
    expect(await screen.findByText('Report')).toBeInTheDocument();
  });
});
