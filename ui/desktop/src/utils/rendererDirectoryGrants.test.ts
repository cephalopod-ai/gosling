import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import { RendererDirectoryGrantRegistry } from './rendererDirectoryGrants';

const temporaryDirectories: string[] = [];

function fixture() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-renderer-grants-'));
  temporaryDirectories.push(directory);
  const approved = path.join(directory, 'approved');
  const denied = path.join(directory, 'denied');
  fs.mkdirSync(approved);
  fs.mkdirSync(denied);
  return {
    approved,
    denied,
    storePath: path.join(directory, 'config', 'renderer-directory-grants.json'),
  };
}

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

describe('RendererDirectoryGrantRegistry', () => {
  it('keeps persisted picker history main-only and grants only the selecting renderer', () => {
    const { approved, denied, storePath } = fixture();
    const registry = new RendererDirectoryGrantRegistry(storePath);

    expect(registry.isGrantedDirectory(10, denied)).toBe(false);
    registry.grantSelectedPath(10, approved);
    expect(registry.isGrantedDirectory(10, approved)).toBe(true);
    expect(registry.isGrantedDirectory(11, approved)).toBe(false);
    expect(registry.isGrantedDirectory(0, approved)).toBe(true);
    expect(registry.isGrantedDirectory(10, denied)).toBe(false);

    const reloaded = new RendererDirectoryGrantRegistry(storePath);
    reloaded.load();
    expect(reloaded.isGrantedDirectory(99, approved)).toBe(false);
    expect(reloaded.isGrantedDirectory(0, approved)).toBe(true);
    expect(reloaded.isGrantedDirectory(99, denied)).toBe(false);
  });

  it('keeps trusted launch roots transient and scoped to one renderer', () => {
    const { approved, storePath } = fixture();
    const registry = new RendererDirectoryGrantRegistry(storePath);

    registry.grantSelectedPath(10, approved, false);
    expect(registry.isGrantedDirectory(10, approved)).toBe(true);
    expect(registry.isGrantedDirectory(11, approved)).toBe(false);
    registry.clearTransient(10);
    expect(registry.isGrantedDirectory(10, approved)).toBe(false);
    expect(fs.existsSync(storePath)).toBe(false);
  });

  it('rejects symlink grant roots', () => {
    if (process.platform === 'win32') return;
    const { approved, storePath } = fixture();
    const symlink = `${approved}-link`;
    fs.symlinkSync(approved, symlink, 'dir');
    const registry = new RendererDirectoryGrantRegistry(storePath);

    expect(() => registry.grantSelectedPath(10, symlink)).toThrow(/Symbolic-link/);
  });
});
