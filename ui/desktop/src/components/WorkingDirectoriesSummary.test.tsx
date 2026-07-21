import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import type { Session } from '../types/session';
import WorkingDirectoriesSummary from './WorkingDirectoriesSummary';

vi.mock('./WorkingDirectoriesMenu', () => ({ default: () => null }));
vi.mock('./bottom_menu/CredentialProfileSelector', () => ({
  CredentialProfileSelector: ({
    credentialProfileName,
    surface,
  }: {
    credentialProfileName?: string | null;
    surface?: string;
  }) => (
    <button
      type="button"
      aria-label={`Credential for this chat: ${credentialProfileName ?? 'No credential'}`}
      data-surface={surface}
    >
      {credentialProfileName ?? 'No credential'}
    </button>
  ),
}));

const session: Session = {
  id: 'session-1',
  name: 'Credential header regression',
  message_count: 1,
  created_at: '2026-07-21T00:00:00Z',
  updated_at: '2026-07-21T00:00:00Z',
  working_dir: '/projects/gosling',
  extension_data: { active: [], installed: [] },
  credential_profile_id: 'profile-1',
  credential_profile_name: 'Team OpenAI',
};

describe('WorkingDirectoriesSummary credential access', () => {
  it('keeps the active chat credential control in the upper-right header', () => {
    render(<WorkingDirectoriesSummary session={session} onSessionChange={vi.fn()} />, {
      wrapper: IntlTestWrapper,
    });

    expect(
      screen.getByRole('button', { name: 'Credential for this chat: Team OpenAI' })
    ).toHaveAttribute('data-surface', 'header');
  });
});
