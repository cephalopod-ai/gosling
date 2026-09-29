import { describe, expect, it } from 'vitest';
import { errorMessage } from './conversionUtils';

describe('errorMessage', () => {
  it("drops Electron's IPC wrapper from a rejected ipcRenderer.invoke", () => {
    expect(
      errorMessage(
        new Error(
          "Error invoking remote method 'copy-artifact-contents': Error: Copy contents supports text files up to 20 MiB. Save a copy for larger files."
        )
      )
    ).toBe('Copy contents supports text files up to 20 MiB. Save a copy for larger files.');
    expect(
      errorMessage(
        new Error("Error invoking remote method 'read-file': TypeError: path must be a string")
      )
    ).toBe('path must be a string');
  });

  it('leaves other messages unchanged', () => {
    expect(errorMessage(new Error('Error: kept as written'))).toBe('Error: kept as written');
    expect(errorMessage({ message: 'plain object' })).toBe('plain object');
    expect(errorMessage(undefined, 'Unknown error')).toBe('Unknown error');
  });
});
