import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import {
  ArtifactWorkbenchProvider,
  useArtifactWorkbench,
} from '../../contexts/ArtifactWorkbenchContext';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { ArtifactMessageLinks } from './ArtifactMessageLinks';

describe('ArtifactMessageLinks', () => {
  it('opens a referenced assistant artifact in the Outputs pane', () => {
    let workbench: ReturnType<typeof useArtifactWorkbench>;

    function Harness() {
      workbench = useArtifactWorkbench();
      return (
        <ArtifactMessageLinks
          content="Review `docs/gcp/build/decision-0006-gate1-review-packet.md`."
          baseDirectory="/workspace"
        />
      );
    }

    render(
      <IntlTestWrapper>
        <ArtifactWorkbenchProvider>
          <Harness />
        </ArtifactWorkbenchProvider>
      </IntlTestWrapper>
    );

    fireEvent.click(
      screen.getByRole('button', {
        name: 'Open decision-0006-gate1-review-packet.md in Outputs',
      })
    );

    expect(workbench!.isOpen).toBe(true);
    expect(workbench!.activeTab?.source).toEqual({
      type: 'file',
      path: 'docs/gcp/build/decision-0006-gate1-review-packet.md',
      baseDirectory: '/workspace',
    });
  });
});
