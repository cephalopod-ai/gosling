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
  it('keeps a picked folder approved across windows and restarts, and nothing else', () => {
    const { approved, denied, storePath } = fixture();
    const registry = new RendererDirectoryGrantRegistry(storePath);

    expect(registry.isGrantedDirectory(10, denied)).toBe(false);
    registry.grantSelectedPath(10, approved);
    expect(registry.isGrantedDirectory(10, approved)).toBe(true);
    expect(registry.isGrantedDirectory(11, approved)).toBe(true);
    expect(registry.isGrantedDirectory(0, approved)).toBe(true);
    expect(registry.isGrantedDirectory(10, denied)).toBe(false);

    // A restart used to drop the stored roots for every window, which is why the
    // app re-prompted for the same folders on every launch.
    const reloaded = new RendererDirectoryGrantRegistry(storePath);
    reloaded.load();
    expect(reloaded.isGrantedDirectory(99, approved)).toBe(true);
    expect(reloaded.isGrantedDirectory(0, approved)).toBe(true);
    expect(reloaded.isGrantedDirectory(99, denied)).toBe(false);
  });

  it('never stores the home directory or a filesystem root', () => {
    const { approved, storePath } = fixture();
    const registry = new RendererDirectoryGrantRegistry(storePath);
    const home = path.resolve(os.homedir());

    registry.grantSelectedPath(10, home);
    registry.grantSelectedPath(10, approved);

    // The picking window still gets what it asked for.
    expect(registry.isGrantedDirectory(10, home)).toBe(true);
    // No other window inherits it, and it does not outlive the run.
    expect(registry.isGrantedDirectory(11, home)).toBe(false);
    const stored = JSON.parse(fs.readFileSync(storePath, 'utf8')) as { roots: string[] };
    expect(stored.roots).toEqual([fs.realpathSync.native(approved)]);
  });

  it('prunes an overly broad root already stored by an earlier version', () => {
    const { approved, storePath } = fixture();
    fs.mkdirSync(path.dirname(storePath), { recursive: true });
    fs.writeFileSync(
      storePath,
      JSON.stringify({
        schemaVersion: 1,
        roots: [path.resolve(os.homedir()), path.parse(approved).root, approved],
      })
    );

    const registry = new RendererDirectoryGrantRegistry(storePath);
    registry.load();

    expect(registry.isGrantedDirectory(99, approved)).toBe(true);
    expect(registry.isGrantedDirectory(99, path.resolve(os.homedir()))).toBe(false);
    const stored = JSON.parse(fs.readFileSync(storePath, 'utf8')) as { roots: string[] };
    expect(stored.roots).toEqual([fs.realpathSync.native(approved)]);
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
