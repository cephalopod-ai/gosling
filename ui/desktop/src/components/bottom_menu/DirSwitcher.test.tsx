import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../i18n/test-utils';
import { DirSwitcher } from './DirSwitcher';

describe('DirSwitcher chat information trigger', () => {
  it('opens chat information instead of the directory menu when configured for a session', () => {
    render(
      <DirSwitcher
        className=""
        sessionId="session-1"
        workingDir="/projects/cephalopod-ai"
        renderChatInfo={(close) => (
          <div role="region" aria-label="Chat information">
            Chat info
            <button type="button" onClick={close}>
              Collapse
            </button>
          </div>
        )}
      />,
      { wrapper: IntlTestWrapper }
    );

    fireEvent.click(
      screen.getByRole('button', { name: 'Open chat information for cephalopod-ai' })
    );

    expect(screen.getByRole('region', { name: 'Chat information' })).toBeInTheDocument();
    expect(screen.queryByText('Current directory')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Collapse' }));
    expect(screen.queryByRole('region', { name: 'Chat information' })).not.toBeInTheDocument();
  });
});
