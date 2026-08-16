import { describe, it, expect } from 'vitest';
import { deriveLoadingStatus, getToolResultError } from './ToolCallWithResponse';
import type { ToolResponseMessageContent } from '../types/message';

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
