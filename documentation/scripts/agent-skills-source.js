const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const SOURCE_REPO = 'https://github.com/block/Agent-Skills.git';
// This snapshot matches all five skills already published in the checked-in manifest.
const SOURCE_COMMIT = '329a55d1500748a0a48d64629757cbe2622e34bf';
const MAX_SOURCE_BYTES = 64 * 1024 * 1024;
const MAX_FILE_BYTES = 2 * 1024 * 1024;
const MAX_SOURCE_FILES = 4096;

function validateSourceTree(directory) {
  let bytes = 0;
  let files = 0;
  function walk(current, depth = 0) {
    if (depth > 16) throw new Error('Skill source exceeds the directory-depth limit');
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (current === directory && entry.name === '.git') continue;
      const file = path.join(current, entry.name);
      if (entry.isSymbolicLink()) throw new Error('Skill source contains a symbolic link');
      if (entry.isDirectory()) {
        walk(file, depth + 1);
      } else if (entry.isFile()) {
        const size = fs.statSync(file).size;
        bytes += size;
        files += 1;
        if (size > MAX_FILE_BYTES || bytes > MAX_SOURCE_BYTES || files > MAX_SOURCE_FILES) {
          throw new Error('Skill source exceeds publication bounds');
        }
        if (entry.name === 'SKILL.md') {
          const text = new TextDecoder('utf-8', { fatal: true }).decode(fs.readFileSync(file));
          if (!/^---\r?\n/.test(text)) throw new Error('Skill source must use YAML frontmatter');
        }
      } else {
        throw new Error('Skill source contains an unsupported file type');
      }
    }
  }
  walk(directory);
}

function preparePinnedSource(directory) {
  const git = (args) => execFileSync('git', [
    '-c', 'credential.helper=', '-c', `core.hooksPath=${process.platform === 'win32' ? 'NUL' : '/dev/null'}`,
    ...args,
  ], { encoding: 'utf8', stdio: 'pipe', timeout: 60000 });
  const cloned = !fs.existsSync(directory);
  if (cloned) {
    fs.mkdirSync(path.dirname(directory), { recursive: true });
    git(['clone', '--no-checkout', '--depth', '1', SOURCE_REPO, directory]);
  } else if (!fs.existsSync(path.join(directory, '.git'))) {
    throw new Error('Existing skill-source directory is not an owned Git checkout');
  }
  const status = git(['-C', directory, 'status', '--porcelain', '--untracked-files=all', '--ignored']).trim();
  if (status && (!cloned || status.split('\n').some((line) => line.startsWith('??') || line.startsWith('!!')))) {
    throw new Error('Skill-source checkout has uncommitted work');
  }
  git(['-C', directory, 'fetch', '--no-tags', '--depth', '1', SOURCE_REPO, SOURCE_COMMIT]);
  git(['-C', directory, 'checkout', '--detach', SOURCE_COMMIT]);
  if (git(['-C', directory, 'rev-parse', 'HEAD']).trim() !== SOURCE_COMMIT) {
    throw new Error('Skill-source checkout does not match the pinned commit');
  }
  validateSourceTree(directory);
  return SOURCE_COMMIT;
}

module.exports = { SOURCE_COMMIT, preparePinnedSource, validateSourceTree };
