import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { SessionArtifactDto } from '@repo-makeover/gosling-sdk';
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
    const { openContent, openFile, setVisibleSession } = useArtifactWorkbench();
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
        <button
          type="button"
          onClick={() => {
            const artifacts: SessionArtifactDto[] = [
              'report.md',
              'analysis.py',
              'engine.rs',
              'build.sh',
            ].map((displayPath, index) => ({
              sessionId: 'session-four-files',
              displayPath,
              resolvedPath: `/outputs/${displayPath}`,
              baseWorkingDir: '/outputs',
              relation: 'created' as const,
              provenance: 'built_in_tool' as const,
              sourceId: `tool-${index}`,
              firstSeenAt: `2026-01-01T00:00:0${index}Z`,
              lastSeenAt: `2026-01-01T00:00:0${index}Z`,
            }));
            setVisibleSession(
              'session-four-files',
              artifacts.concat({
                sessionId: 'session-four-files',
                displayPath: 'David.Casbeer@us.af.mil',
                resolvedPath: '/outputs/David.Casbeer@us.af.mil',
                baseWorkingDir: '/outputs',
                relation: 'referenced',
                provenance: 'compatibility_inference',
                sourceId: 'assistant-message',
                firstSeenAt: '2026-01-01T00:00:04Z',
                lastSeenAt: '2026-01-01T00:00:04Z',
              })
            );
          }}
        >
          Load four outputs
        </button>
        <button
          type="button"
          onClick={() =>
            setVisibleSession('session-pdf', [
              {
                sessionId: 'session-pdf',
                displayPath: 'report.pdf',
                resolvedPath: '/outputs/report.pdf',
                baseWorkingDir: '/outputs',
                mimeType: 'application/pdf; version=1.7',
                relation: 'created',
                provenance: 'built_in_tool',
                firstSeenAt: '2026-01-01T00:00:00Z',
                lastSeenAt: '2026-01-01T00:00:00Z',
              },
            ])
          }
        >
          Load MIME PDF
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
      setVisibleSessionArtifacts: vi.fn(),
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

  it('offers to grant access when a preview stays blocked after retrying', async () => {
    const selectArtifactFile = vi.fn().mockResolvedValue('/outputs/report.md');
    Object.assign(window.electron, { selectArtifactFile });

    render(
      <IntlTestWrapper>
        <ArtifactWorkbenchProvider>
          <Harness />
        </ArtifactWorkbenchProvider>
      </IntlTestWrapper>
    );

    fireEvent.click(screen.getByRole('button', { name: 'Load four outputs' }));
    fireEvent.click(screen.getByTitle('/outputs/report.md'));

    const grantButton = await screen.findByRole(
      'button',
      {
        name: 'Select this file to grant access and preview it',
      },
      { timeout: 2_000 }
    );
    fireEvent.click(grantButton);

    await waitFor(() => expect(selectArtifactFile).toHaveBeenCalledWith('report.md'));
  });

  it('shows four discovered outputs without opening or reading a preview', () => {
    render(
      <IntlTestWrapper>
        <ArtifactWorkbenchProvider>
          <Harness />
        </ArtifactWorkbenchProvider>
      </IntlTestWrapper>
    );

    fireEvent.click(screen.getByRole('button', { name: 'Load four outputs' }));

    expect(screen.getByText('Outputs 4')).toBeInTheDocument();
    expect(screen.getByText('report.md')).toBeInTheDocument();
    expect(screen.getByText('analysis.py')).toBeInTheDocument();
    expect(screen.getByText('engine.rs')).toBeInTheDocument();
    expect(screen.getByText('build.sh')).toBeInTheDocument();
    expect(screen.queryByText('David.Casbeer@us.af.mil')).not.toBeInTheDocument();
    expect(
      screen.queryByText('This file type does not have an in-app preview yet.')
    ).not.toBeInTheDocument();
    expect(readArtifactFile).not.toHaveBeenCalled();
  });

  it('keeps a supported PDF visible when its metadata includes a parameterized MIME type', () => {
    render(
      <IntlTestWrapper>
        <ArtifactWorkbenchProvider>
          <Harness />
        </ArtifactWorkbenchProvider>
      </IntlTestWrapper>
    );

    fireEvent.click(screen.getByRole('button', { name: 'Load MIME PDF' }));

    expect(screen.getByText('Outputs 1')).toBeInTheDocument();
    expect(screen.getByText('report.pdf')).toBeInTheDocument();
    expect(readArtifactFile).not.toHaveBeenCalled();
  });
});
