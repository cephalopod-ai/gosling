import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import { ArtifactWorkbenchProvider, useArtifactWorkbench } from './ArtifactWorkbenchContext';

type Workbench = ReturnType<typeof useArtifactWorkbench>;

describe('ArtifactWorkbenchProvider', () => {
  let workbench: Workbench;

  function Harness() {
    workbench = useArtifactWorkbench();
    return null;
  }

  beforeEach(() => {
    localStorage.clear();
  });

  it('opens local files and transient tool outputs without a source-specific contract', () => {
    render(
      <ArtifactWorkbenchProvider>
        <Harness />
      </ArtifactWorkbenchProvider>
    );

    act(() => workbench.openFile('deliverables/brief.md', '/workspace'));
    expect(workbench.isOpen).toBe(true);
    expect(workbench.activeTab?.kind).toBe('markdown');

    act(() =>
      workbench.openContent({
        title: 'Tool output',
        content: '{"ok":true}',
        mimeType: 'application/json',
      })
    );
    expect(workbench.tabs).toHaveLength(2);
    expect(workbench.activeTab?.kind).toBe('json');
  });

  it('keeps the same relative output path distinct across unrelated working directories', () => {
    render(
      <ArtifactWorkbenchProvider>
        <Harness />
      </ArtifactWorkbenchProvider>
    );

    act(() => {
      workbench.openFile('output/report.md', '/projects/alpha');
      workbench.openFile('output/report.md', '/projects/beta');
    });

    expect(workbench.tabs).toHaveLength(2);
    expect(workbench.tabs.map((tab) => tab.source)).toEqual([
      { type: 'file', path: 'output/report.md', baseDirectory: '/projects/alpha' },
      { type: 'file', path: 'output/report.md', baseDirectory: '/projects/beta' },
    ]);
  });

  it('persists file tabs but not transient content', async () => {
    render(
      <ArtifactWorkbenchProvider>
        <Harness />
      </ArtifactWorkbenchProvider>
    );

    act(() => {
      workbench.openFile('/workspace/report.csv');
      workbench.openContent({ title: 'Log', content: 'done' });
    });

    await waitFor(() => {
      const stored = JSON.parse(localStorage.getItem('gosling-artifact-workbench-v1') ?? '{}');
      expect(stored.tabs).toHaveLength(1);
      expect(stored.tabs[0].source.path).toBe('/workspace/report.csv');
    });
  });
});
