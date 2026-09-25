import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import WebsiteLoginsSection from './WebsiteLoginsSection';
import {
  deleteWebsiteLogin,
  listWebsiteLogins,
  saveWebsiteLogin,
  type WebsiteLoginDto,
} from '../../../acp/websiteLogins';
import { IntlTestWrapper } from '../../../i18n/test-utils';

vi.mock('../../../acp/websiteLogins', () => ({
  listWebsiteLogins: vi.fn(),
  saveWebsiteLogin: vi.fn(),
  deleteWebsiteLogin: vi.fn(),
}));

vi.mock('react-toastify', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

const mockedList = vi.mocked(listWebsiteLogins);
const mockedSave = vi.mocked(saveWebsiteLogin);
const mockedDelete = vi.mocked(deleteWebsiteLogin);

const github: WebsiteLoginDto = {
  id: 'login-1',
  name: 'Work GitHub',
  url: 'https://github.com/login',
  username: 'octocat',
  hasPassword: true,
  placeholder: '{{login:Work GitHub}}',
};

const renderSection = () => render(<WebsiteLoginsSection />, { wrapper: IntlTestWrapper });

describe('WebsiteLoginsSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedSave.mockResolvedValue(github);
    mockedDelete.mockResolvedValue();
  });

  it('shows each login as one card with its website, username and agent placeholder', async () => {
    mockedList.mockResolvedValue([github]);
    renderSection();

    const card = await screen.findByTestId('website-login-card');
    expect(within(card).getByText('Work GitHub')).toBeInTheDocument();
    expect(within(card).getByText('https://github.com/login')).toBeInTheDocument();
    expect(within(card).getByText('octocat')).toBeInTheDocument();
    expect(within(card).getByText('{{login:Work GitHub}}')).toBeInTheDocument();
    expect(within(card).getByText('••••••••')).toBeInTheDocument();
  });

  it('saves a new login with website, username and password together', async () => {
    const user = userEvent.setup();
    mockedList.mockResolvedValueOnce([]).mockResolvedValueOnce([github]);
    renderSection();

    await user.click(await screen.findByRole('button', { name: 'Add login' }));
    const form = screen.getByTestId('website-login-form');
    await user.type(within(form).getByLabelText('Name'), 'Work GitHub');
    await user.type(within(form).getByLabelText('Website'), 'https://github.com/login');
    await user.type(within(form).getByLabelText('Username or email'), 'octocat');
    await user.type(within(form).getByLabelText('Password'), 'hunter2');
    await user.click(within(form).getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(mockedSave).toHaveBeenCalledWith({
        id: undefined,
        name: 'Work GitHub',
        url: 'https://github.com/login',
        username: 'octocat',
        password: 'hunter2',
      })
    );
    expect(await screen.findByTestId('website-login-card')).toBeInTheDocument();
  });

  it('keeps the saved password when editing without typing a new one', async () => {
    const user = userEvent.setup();
    mockedList.mockResolvedValue([github]);
    renderSection();

    await user.click(await screen.findByRole('button', { name: 'Edit Work GitHub' }));
    const form = screen.getByTestId('website-login-form');
    const username = within(form).getByLabelText('Username or email');
    await user.clear(username);
    await user.type(username, 'monalisa');
    await user.click(within(form).getByRole('button', { name: 'Save' }));

    await waitFor(() =>
      expect(mockedSave).toHaveBeenCalledWith({
        id: 'login-1',
        name: 'Work GitHub',
        url: 'https://github.com/login',
        username: 'monalisa',
        password: undefined,
      })
    );
  });

  it('deletes a login after confirmation', async () => {
    const user = userEvent.setup();
    mockedList.mockResolvedValueOnce([github]).mockResolvedValueOnce([]);
    renderSection();

    await user.click(await screen.findByRole('button', { name: 'Delete Work GitHub' }));
    await user.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => expect(mockedDelete).toHaveBeenCalledWith('login-1'));
    expect(await screen.findByText('No website logins saved yet.')).toBeInTheDocument();
  });
});
