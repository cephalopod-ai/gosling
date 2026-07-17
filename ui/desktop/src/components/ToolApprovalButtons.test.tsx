import { render, type RenderOptions, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveAcpPermissionRequest } from '../acp/permissionRequests';
import { listTools, setToolPermissions } from '../acp/permissions';
import { IntlTestWrapper } from '../i18n/test-utils';
import ToolApprovalButtons from './ToolApprovalButtons';

vi.mock('../acp/permissionRequests', () => ({
  resolveAcpPermissionRequest: vi.fn(),
}));

vi.mock('../acp/permissions', () => ({
  listTools: vi.fn(),
  setToolPermissions: vi.fn(),
}));

const renderWithIntl = (ui: React.ReactElement, options?: RenderOptions) =>
  render(ui, { wrapper: IntlTestWrapper, ...options });

const resolveAcpPermissionRequestMock = vi.mocked(resolveAcpPermissionRequest);
const listToolsMock = vi.mocked(listTools);
const setToolPermissionsMock = vi.mocked(setToolPermissions);

describe('ToolApprovalButtons', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('marks the approval accepted when the ACP request resolves', async () => {
    resolveAcpPermissionRequestMock.mockReturnValueOnce(true);

    renderWithIntl(
      <ToolApprovalButtons
        data={{
          id: 'tool-call-approved',
          toolName: 'developer__shell',
          sessionId: 'session-1',
        }}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Allow Once' }));

    expect(resolveAcpPermissionRequestMock).toHaveBeenCalledWith(
      'session-1',
      'tool-call-approved',
      'allow_once'
    );
    expect(screen.getByText('developer__shell - Allowed once')).toBeInTheDocument();
  });

  it('shows a stale request error when ACP has no pending request', async () => {
    resolveAcpPermissionRequestMock.mockReturnValueOnce(false);

    renderWithIntl(
      <ToolApprovalButtons
        data={{
          id: 'tool-call-rerun',
          toolName: 'developer__shell',
          sessionId: 'session-1',
        }}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Allow Once' }));

    expect(resolveAcpPermissionRequestMock).toHaveBeenCalledWith(
      'session-1',
      'tool-call-rerun',
      'allow_once'
    );
    expect(screen.getByText('This approval request is no longer active.')).toBeInTheDocument();
    expect(screen.queryByText('developer__shell - Allowed once')).not.toBeInTheDocument();
  });

  it('does not mutate extension permissions when the approval request is stale', async () => {
    resolveAcpPermissionRequestMock.mockReturnValueOnce(false);

    renderWithIntl(
      <ToolApprovalButtons
        data={{
          id: 'tool-call-stale',
          toolName: 'developer__shell',
          sessionId: 'session-1',
        }}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Always Allow all developer tools' }));

    expect(resolveAcpPermissionRequestMock).toHaveBeenCalledWith(
      'session-1',
      'tool-call-stale',
      'always_allow'
    );
    expect(listToolsMock).not.toHaveBeenCalled();
    expect(setToolPermissionsMock).not.toHaveBeenCalled();
    expect(screen.getByText('This approval request is no longer active.')).toBeInTheDocument();
    expect(
      screen.queryByText('developer__shell - Always allowed (developer tools)')
    ).not.toBeInTheDocument();
  });

  it('validates the approval request before mutating extension permissions', async () => {
    const callOrder: string[] = [];
    resolveAcpPermissionRequestMock.mockImplementationOnce(() => {
      callOrder.push('resolve');
      return true;
    });
    listToolsMock.mockImplementationOnce(async () => {
      callOrder.push('listTools');
      return [];
    });
    setToolPermissionsMock.mockImplementationOnce(async () => {
      callOrder.push('setToolPermissions');
    });

    renderWithIntl(
      <ToolApprovalButtons
        data={{
          id: 'tool-call-live',
          toolName: 'developer__shell',
          sessionId: 'session-1',
        }}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Always Allow all developer tools' }));

    expect(callOrder).toEqual(['resolve', 'listTools', 'setToolPermissions']);
    expect(resolveAcpPermissionRequestMock).toHaveBeenCalledWith(
      'session-1',
      'tool-call-live',
      'always_allow'
    );
    expect(setToolPermissionsMock).toHaveBeenCalledWith([
      { toolName: 'developer__shell', permission: 'always_allow' },
    ]);
    expect(
      screen.getByText('developer__shell - Always allowed (developer tools)')
    ).toBeInTheDocument();
  });
});
