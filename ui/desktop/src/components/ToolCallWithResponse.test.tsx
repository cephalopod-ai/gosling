import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import { IntlTestWrapper } from '../i18n/test-utils';
import ToolCallWithResponse, {
  deriveLoadingStatus,
  getToolResultError,
} from './ToolCallWithResponse';
import type { ToolRequestMessageContent, ToolResponseMessageContent } from '../types/message';

vi.mock('../contexts/ArtifactWorkbenchContext', () => ({
  useArtifactWorkbench: () => ({ openFile: vi.fn() }),
}));

function toolResponse(status?: string): ToolResponseMessageContent {
  return {
    type: 'toolResponse',
    id: 'req-1',
    toolResult: status ? { status } : {},
  } as unknown as ToolResponseMessageContent;
}

describe('deriveLoadingStatus', () => {
  it('is loading while streaming is still in progress and no response arrived', () => {
    expect(deriveLoadingStatus(undefined, true)).toBe('loading');
  });

  it('is unknown when streaming finished but no response ever arrived', () => {
    // This is the regression case: a dropped connection or a backend that
    // never sends a tool response must not be reported as a green success.
    expect(deriveLoadingStatus(undefined, false)).toBe('unknown');
  });

  it('is success once a non-error response arrives', () => {
    expect(deriveLoadingStatus(toolResponse(), true)).toBe('success');
    expect(deriveLoadingStatus(toolResponse(), false)).toBe('success');
  });

  it('is error when the response reports an error status', () => {
    expect(deriveLoadingStatus(toolResponse('error'), false)).toBe('error');
  });
});

// The `error` string was parsed for the status icon but never rendered, so a
// failed tool showed no reason (WFG-GOS-003).
describe('getToolResultError', () => {
  it('returns the error string from a failed tool result', () => {
    expect(getToolResultError({ status: 'error', error: 'ENOENT: missing file' })).toBe(
      'ENOENT: missing file'
    );
  });

  it('never returns an empty string for a failed result', () => {
    expect(getToolResultError({ status: 'error', error: '   ' })).toBe(
      'The tool reported an error with no message.'
    );
    expect(getToolResultError({ status: 'error' })).toBe(
      'The tool reported an error with no message.'
    );
  });

  it('returns nothing for a successful or absent result', () => {
    expect(getToolResultError({ status: 'success', value: { content: [] } })).toBeUndefined();
    expect(getToolResultError(undefined)).toBeUndefined();
    expect(getToolResultError(null)).toBeUndefined();
  });
});

// A turn stopped while a tool call waited (quit during an approval, a killed
// process) is closed by the backend with this result on the next load, so the
// card must say the call never ran instead of staying pending or "failed".
describe('a replayed tool call', () => {
  const request = {
    type: 'toolRequest',
    id: 'call-1',
    toolCall: { status: 'success', value: { name: 'developer__shell', arguments: {} } },
  } as unknown as ToolRequestMessageContent;

  function failed(error: string): ToolResponseMessageContent {
    return {
      type: 'toolResponse',
      id: 'call-1',
      toolResult: { status: 'error', error },
    } as unknown as ToolResponseMessageContent;
  }

  async function renderExpanded(toolResponse: ToolResponseMessageContent) {
    render(
      <ToolCallWithResponse
        isCancelledMessage={false}
        toolRequest={request}
        toolResponse={toolResponse}
        isPendingApproval={false}
      />,
      { wrapper: IntlTestWrapper }
    );
    await userEvent.click(screen.getByRole('button'));
  }

  it('shows a call the closed turn never ran as not run', async () => {
    await renderExpanded(
      failed(
        '-32600: Tool execution was cancelled before it started because the prior turn ended. It will not be retried automatically.'
      )
    );

    expect(screen.getByLabelText('Tool status: not run')).toBeInTheDocument();
    expect(
      screen.getByText('Not run — the run ended before this tool started')
    ).toBeInTheDocument();
    expect(screen.queryByText('Tool failed')).not.toBeInTheDocument();
  });

  it('still shows a real failure as failed', async () => {
    await renderExpanded(failed('ENOENT: missing file'));

    expect(screen.getByLabelText('Tool status: error')).toBeInTheDocument();
    expect(screen.getByText('Tool failed')).toBeInTheDocument();
    expect(screen.getByText('ENOENT: missing file')).toBeInTheDocument();
  });
});
