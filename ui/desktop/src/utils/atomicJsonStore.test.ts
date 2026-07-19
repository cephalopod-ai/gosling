import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { readJsonFileWithRecoverySync, writeJsonFileAtomicSync } from './atomicJsonStore';

interface TestDocument {
  revision: number;
}

function isTestDocument(value: unknown): value is TestDocument {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    typeof (value as Record<string, unknown>).revision === 'number'
  );
}

const temporaryDirectories: string[] = [];

function temporaryFile(): string {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-atomic-json-'));
  temporaryDirectories.push(directory);
  return path.join(directory, 'state.json');
}

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

describe('atomic JSON storage', () => {
  it('writes a complete owner-only document and retains a previous-good snapshot', () => {
    const filePath = temporaryFile();
    writeJsonFileAtomicSync(filePath, { revision: 1 });
    writeJsonFileAtomicSync(filePath, { revision: 2 });

    expect(JSON.parse(fs.readFileSync(filePath, 'utf8'))).toEqual({ revision: 2 });
    expect(JSON.parse(fs.readFileSync(`${filePath}.previous`, 'utf8'))).toEqual({ revision: 1 });
    if (process.platform !== 'win32') {
      expect(fs.statSync(filePath).mode & 0o777).toBe(0o600);
      expect(fs.statSync(path.dirname(filePath)).mode & 0o777).toBe(0o700);
    }
    expect(fs.readdirSync(path.dirname(filePath)).some((name) => name.endsWith('.tmp'))).toBe(
      false
    );
  });

  it('recovers malformed current data from the previous-good snapshot', () => {
    const filePath = temporaryFile();
    writeJsonFileAtomicSync(filePath, { revision: 1 });
    writeJsonFileAtomicSync(filePath, { revision: 2 });
    fs.writeFileSync(filePath, '{broken', 'utf8');

    const recovered = readJsonFileWithRecoverySync(filePath, isTestDocument);

    expect(recovered?.value).toEqual({ revision: 1 });
    expect(recovered?.recoveredFromPrevious).toBe(true);
    expect(recovered?.corruptFilePath).toBeTruthy();
    expect(JSON.parse(fs.readFileSync(filePath, 'utf8'))).toEqual({ revision: 1 });
  });

  it('does not overwrite the previous-good snapshot with malformed data', () => {
    const filePath = temporaryFile();
    writeJsonFileAtomicSync(filePath, { revision: 1 });
    writeJsonFileAtomicSync(filePath, { revision: 2 });
    fs.writeFileSync(filePath, '{broken', 'utf8');

    writeJsonFileAtomicSync(filePath, { revision: 3 });

    expect(JSON.parse(fs.readFileSync(`${filePath}.previous`, 'utf8'))).toEqual({ revision: 1 });
  });

  it('quarantines malformed data when no previous-good snapshot exists', () => {
    const filePath = temporaryFile();
    fs.writeFileSync(filePath, '{broken', 'utf8');

    expect(() => readJsonFileWithRecoverySync(filePath, isTestDocument)).toThrow();

    expect(fs.existsSync(filePath)).toBe(false);
    expect(
      fs.readdirSync(path.dirname(filePath)).some((name) => name.startsWith('state.json.corrupt-'))
    ).toBe(true);
  });

  it('quarantines malformed current data when the previous snapshot is also malformed', () => {
    const filePath = temporaryFile();
    fs.writeFileSync(filePath, '{broken', 'utf8');
    fs.writeFileSync(`${filePath}.previous`, '{also-broken', 'utf8');

    expect(() => readJsonFileWithRecoverySync(filePath, isTestDocument)).toThrow();

    expect(fs.existsSync(filePath)).toBe(false);
  });
});
