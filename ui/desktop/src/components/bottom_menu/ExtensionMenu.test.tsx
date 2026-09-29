import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { IntlTestWrapper } from '../../i18n/test-utils';
import type { FixedExtensionEntry } from '../ConfigContext';
import { ExtensionMenu } from './ExtensionMenu';

const extension = (name: string, enabled: boolean) =>
  ({ name, type: 'stdio', cmd: 'false', args: [], enabled }) as unknown as FixedExtensionEntry;

describe('ExtensionMenu', () => {
  it('marks an extension that failed to start in this chat with its cause', () => {
    render(
      <ExtensionMenu
        extensions={[extension('developer', true), extension('broken', false)]}
        title="manage extensions"
        searchPlaceholder="search"
        description="Extensions for this chat session"
        emptyMessage="none"
        noResultsMessage="none"
        hidden={false}
        isTransitioning={false}
        isSortPending={false}
        togglingExtensionName={null}
        loadFailures={new Map([['broken', 'process quit with exit status: 1']])}
        onToggle={vi.fn()}
      />,
      { wrapper: IntlTestWrapper }
    );

    fireEvent.pointerDown(screen.getByTitle('manage extensions'), { button: 0, ctrlKey: false });

    const marker = screen.getByText('Failed to start: process quit with exit status: 1');
    expect(marker).toHaveAttribute('title', 'process quit with exit status: 1');
    expect(screen.getAllByText(/Failed to start/)).toHaveLength(1);
  });
});
