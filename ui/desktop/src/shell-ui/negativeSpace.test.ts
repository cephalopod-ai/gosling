import fs from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = path.dirname(new URL(import.meta.url).pathname);

function sourceFiles(): string[] {
  const files: string[] = [];
  const walk = (directory: string) => {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const full = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        walk(full);
        continue;
      }
      if (!/\.tsx?$/.test(entry.name)) continue;
      if (/\.test\.tsx?$/.test(entry.name)) continue;
      if (entry.name === 'fakeShellApi.ts' || entry.name === 'testSupport.ts') continue;
      files.push(full);
    }
  };
  walk(ROOT);
  return files;
}

interface ImportStatement {
  typeOnly: boolean;
  source: string;
}

/**
 * Import statements are extracted whole rather than line by line, because a multi-line
 * `import type { A, B } from '...'` would otherwise look like an untyped import of its last line.
 */
function importStatements(contents: string): ImportStatement[] {
  const statements: ImportStatement[] = [];
  const pattern = /import\s+(type\s+)?([^;]*?)from\s+['"]([^'"]+)['"]/gs;
  let match = pattern.exec(contents);
  while (match) {
    const clause = match[2];
    const everySpecifierTyped =
      match[1] !== undefined ||
      (/\{/.test(clause) &&
        clause
          .replace(/[{}]/g, '')
          .split(',')
          .map((entry) => entry.trim())
          .filter((entry) => entry.length > 0)
          .every((entry) => entry.startsWith('type ')));
    statements.push({ typeOnly: everySpecifierTyped, source: match[3] });
    match = pattern.exec(contents);
  }
  return statements;
}

function read(file: string): string {
  return fs.readFileSync(file, 'utf8');
}

/**
 * The renderer's only authority is `window.goslingShell`. These assertions are the executable form
 * of that rule: they fail if a future change reaches for Electron, Node, the network, browser
 * storage, or an absolute path field.
 */
describe('renderer negative space', () => {
  const files = sourceFiles();

  it('contains source files to check', () => {
    expect(files.length).toBeGreaterThan(10);
  });

  it.each([
    ['a CommonJS require', /\brequire\s*\(/],
    ['an Electron import', /from\s+['"]electron['"]/],
    ['ipcRenderer', /ipcRenderer/],
    ['a Node builtin import', /from\s+['"]node:/],
    ['a bare fs or path import', /from\s+['"](fs|path|child_process|os)['"]/],
    ['fetch', /\bfetch\s*\(/],
    ['XMLHttpRequest', /XMLHttpRequest/],
    ['WebSocket', /new\s+WebSocket/],
    ['localStorage', /localStorage/],
    ['sessionStorage', /sessionStorage/],
    ['eval', /\beval\s*\(/],
    ['dangerouslySetInnerHTML', /dangerouslySetInnerHTML/],
    ['innerHTML assignment', /\.innerHTML\s*=/],
    ['a process reference', /\bprocess\.(env|argv|cwd)/],
  ])('never uses %s', (_label, pattern) => {
    const offenders = files.filter((file) => pattern.test(read(file)));
    expect(offenders.map((file) => path.relative(ROOT, file))).toEqual([]);
  });

  it.each([
    ['resolvedPath', /\bresolvedPath\b/],
    ['baseWorkingDir', /\bbaseWorkingDir\b/],
    ['serverSecret', /\bserverSecret\b/],
    ['acpUrl', /\bacpUrl\b/],
    ['a credential secret field', /\b(secretValue|apiKey|accessToken)\b/],
  ])('never references %s', (_label, pattern) => {
    const offenders = files.filter((file) => pattern.test(read(file)));
    expect(offenders.map((file) => path.relative(ROOT, file))).toEqual([]);
  });

  it('reaches window.goslingShell from exactly one module', () => {
    const offenders = files.filter(
      (file) => /goslingShell/.test(read(file)) && path.basename(file) !== 'api.ts'
    );
    expect(offenders.map((file) => path.relative(ROOT, file))).toEqual([]);
  });

  it('imports host shell modules for types only', () => {
    const violations: string[] = [];
    for (const file of files) {
      for (const statement of importStatements(read(file))) {
        if (!/(\.\.\/)+shell\//.test(statement.source)) continue;
        // Only `settingsSchema` may be imported for its values: it is deliberately free of Node and
        // Electron imports so the renderer bundle cannot acquire them transitively.
        if (statement.source.endsWith('/settingsSchema')) continue;
        if (!statement.typeOnly) {
          violations.push(`${path.relative(ROOT, file)} -> ${statement.source}`);
        }
      }
    }
    expect(violations).toEqual([]);
  });

  it('never imports a main-process module as a value', () => {
    const mainProcessModules = new Set([
      'ipcMain',
      'bootstrap',
      'acpRuntime',
      'main',
      'preload',
      'diagnostics',
      'handoff',
      'resources',
      'profile',
      'directoryController',
      'credentialController',
      'sessionController',
      'interactionController',
      'runtimeController',
      'compatibility',
      'localSettings',
      'lifecycle',
      'ipc',
      'operationFailure',
      'runtimeSnapshot',
      'preloadApi',
      'sessionUpdateProjection',
    ]);
    const violations: string[] = [];
    for (const file of files) {
      for (const statement of importStatements(read(file))) {
        const match = /(?:\.\.\/)+shell\/(.+)$/.exec(statement.source);
        if (!match) continue;
        const moduleName = match[1];
        if (!mainProcessModules.has(moduleName)) continue;
        if (!statement.typeOnly) {
          violations.push(`${path.relative(ROOT, file)} -> ${moduleName}`);
        }
      }
    }
    expect(violations).toEqual([]);
  });
});
