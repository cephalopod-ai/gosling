#!/usr/bin/env node
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const projectDir = path.join(__dirname, '..');
const messagesDir = path.join(projectDir, 'src', 'i18n', 'messages');
const sourceHashesPath = path.join(__dirname, 'i18n-source-hashes.json');

function parseJsonWithoutDuplicateKeys(contents, label) {
  const parsed = JSON.parse(contents);
  const stack = [];

  for (let index = 0; index < contents.length; index += 1) {
    const character = contents[index];
    if (character === '"') {
      const start = index;
      index += 1;
      while (index < contents.length) {
        if (contents[index] === '\\') {
          index += 2;
          continue;
        }
        if (contents[index] === '"') break;
        index += 1;
      }

      const context = stack.at(-1);
      if (context?.type === 'object' && context.expectingKey) {
        const key = JSON.parse(contents.slice(start, index + 1));
        if (context.keys.has(key)) {
          throw new SyntaxError(`${label} contains duplicate JSON key ${JSON.stringify(key)}.`);
        }
        context.keys.add(key);
        context.expectingKey = false;
      }
    } else if (character === '{') {
      stack.push({ type: 'object', expectingKey: true, keys: new Set() });
    } else if (character === '[') {
      stack.push({ type: 'array' });
    } else if (character === '}' || character === ']') {
      stack.pop();
    } else if (character === ',' && stack.at(-1)?.type === 'object') {
      stack.at(-1).expectingKey = true;
    }
  }

  return parsed;
}

function assertObject(value, label) {
  if (value === null || Array.isArray(value) || typeof value !== 'object') {
    throw new TypeError(`${label} must be a JSON object.`);
  }
}

function validateMessageCatalog(catalog, label) {
  assertObject(catalog, label);
  for (const [key, message] of Object.entries(catalog)) {
    assertObject(message, `${label} message ${JSON.stringify(key)}`);
    if (typeof message.defaultMessage !== 'string') {
      throw new TypeError(
        `${label} message ${JSON.stringify(key)} must have a string defaultMessage.`
      );
    }
  }
}

function validateSourceHashes(sourceHashes, label) {
  assertObject(sourceHashes, label);
  for (const [key, hash] of Object.entries(sourceHashes)) {
    if (typeof hash !== 'string' || !/^[0-9a-f]{64}$/.test(hash)) {
      throw new TypeError(`${label} hash for ${JSON.stringify(key)} must be a SHA-256 hex string.`);
    }
  }
}

function sourceHashesFor(source) {
  return Object.fromEntries(
    Object.entries(source).map(([key, message]) => [key, sha256(JSON.stringify(message))])
  );
}

function sha256(contents) {
  return crypto.createHash('sha256').update(contents).digest('hex');
}

function cleanSuccessfulRecoveryUnlocked(recoveryDir) {
  if (!fs.existsSync(recoveryDir)) return { preserved: 0, removed: 0 };
  let preserved = 0;
  let removed = 0;

  for (const entry of fs.readdirSync(recoveryDir, { withFileTypes: true })) {
    if (entry.name === '.i18n-sync.lock') continue;
    if (!entry.isDirectory()) {
      preserved += 1;
      continue;
    }
    const transactionPath = path.join(recoveryDir, entry.name);
    const manifestPath = path.join(transactionPath, 'manifest.json');
    if (!fs.existsSync(manifestPath)) {
      preserved += 1;
      continue;
    }

    try {
      const manifest = parseJsonWithoutDuplicateKeys(
        fs.readFileSync(manifestPath, 'utf8'),
        manifestPath
      );
      if (manifest.status !== 'successful') {
        preserved += 1;
        continue;
      }
      fs.rmSync(transactionPath, { recursive: true });
      removed += 1;
    } catch {
      preserved += 1;
    }
  }

  return { preserved, removed };
}

function changedSourceKeys(previousSourceHashes, sourceHashes) {
  return new Set(
    Object.keys(sourceHashes).filter((key) => previousSourceHashes[key] !== sourceHashes[key])
  );
}

function changedExistingSourceKeys(previousSourceHashes, sourceHashes) {
  return new Set(
    Object.keys(sourceHashes).filter(
      (key) =>
        Object.hasOwn(previousSourceHashes, key) && previousSourceHashes[key] !== sourceHashes[key]
    )
  );
}

function removedSourceKeys(previousSourceHashes, sourceHashes) {
  return new Set(
    Object.keys(previousSourceHashes).filter((key) => !Object.hasOwn(sourceHashes, key))
  );
}

function synchronizeLocale(source, locale) {
  const synchronized = Object.fromEntries(
    Object.entries(locale).filter(([key]) => Object.hasOwn(source, key))
  );

  for (const [key, message] of Object.entries(source)) {
    if (!Object.hasOwn(synchronized, key)) {
      synchronized[key] = message;
    }
  }

  return synchronized;
}

function assertFilesUnchanged(files) {
  for (const file of files) {
    if (fs.readFileSync(file.path, 'utf8') !== file.original) {
      throw new Error(`Refusing to overwrite concurrently modified catalog: ${file.path}`);
    }
  }
}

function installWithoutOverwrite(source, destination) {
  fs.linkSync(source, destination);
  fs.unlinkSync(source);
}

function fsyncPath(target) {
  let descriptor;
  try {
    descriptor = fs.openSync(target, 'r');
    fs.fsyncSync(descriptor);
  } catch (error) {
    if (!['EBADF', 'EINVAL', 'ENOTSUP'].includes(error.code)) throw error;
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
  }
}

function writeJsonDurably(target, value) {
  const tempPath = `${target}.write-${process.pid}-${crypto.randomUUID()}`;
  try {
    fs.writeFileSync(tempPath, `${JSON.stringify(value, null, 2)}\n`, { flag: 'wx' });
    fsyncPath(tempPath);
    fs.renameSync(tempPath, target);
    fsyncPath(path.dirname(target));
  } finally {
    if (fs.existsSync(tempPath)) fs.rmSync(tempPath);
  }
}

function processIsAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error.code !== 'ESRCH';
  }
}

function withRecoveryLock(recoveryDir, operation) {
  fs.mkdirSync(recoveryDir, { recursive: true });
  const lockPath = path.join(recoveryDir, '.i18n-sync.lock');
  const token = crypto.randomUUID();

  while (true) {
    try {
      fs.mkdirSync(lockPath);
      try {
        writeJsonDurably(path.join(lockPath, 'owner.json'), {
          acquiredAt: new Date().toISOString(),
          pid: process.pid,
          token,
        });
        fsyncPath(recoveryDir);
      } catch (initializationError) {
        fs.rmSync(lockPath, { recursive: true, force: true });
        initializationError.lockInitializationFailed = true;
        throw initializationError;
      }
      break;
    } catch (error) {
      if (error.lockInitializationFailed) throw error;
      if (error.code !== 'EEXIST') throw error;
      let stale = false;
      let owner = null;
      try {
        owner = parseJsonWithoutDuplicateKeys(
          fs.readFileSync(path.join(lockPath, 'owner.json'), 'utf8'),
          path.join(lockPath, 'owner.json')
        );
        stale = !processIsAlive(owner.pid);
      } catch {
        const age = Date.now() - fs.statSync(lockPath).mtimeMs;
        stale = age > 30_000;
      }
      if (!stale) {
        throw new Error(
          `Another i18n synchronization is running${owner?.pid ? ` (pid ${owner.pid})` : ''}.`
        );
      }

      const stalePath = `${lockPath}.stale-${crypto.randomUUID()}`;
      try {
        fs.renameSync(lockPath, stalePath);
        fs.rmSync(stalePath, { recursive: true, force: true });
        fsyncPath(recoveryDir);
      } catch (takeoverError) {
        if (!['ENOENT', 'EEXIST'].includes(takeoverError.code)) throw takeoverError;
      }
    }
  }

  try {
    return operation();
  } finally {
    let ownsLock = false;
    try {
      const owner = JSON.parse(fs.readFileSync(path.join(lockPath, 'owner.json'), 'utf8'));
      ownsLock = owner.token === token;
    } catch {
      ownsLock = false;
    }
    if (ownsLock) {
      fs.rmSync(lockPath, { recursive: true, force: true });
      fsyncPath(recoveryDir);
      if (!fs.readdirSync(recoveryDir).length) fs.rmdirSync(recoveryDir);
    }
  }
}

function cleanSuccessfulRecovery(recoveryDir) {
  return withRecoveryLock(recoveryDir, () => cleanSuccessfulRecoveryUnlocked(recoveryDir));
}

function pathIsInside(candidate, parent) {
  const relative = path.relative(path.resolve(parent), path.resolve(candidate));
  return relative !== '' && !relative.startsWith(`..${path.sep}`) && relative !== '..';
}

function recoverInterruptedTransactionsUnlocked(recoveryDir, { messagesDir, sourceHashesPath }) {
  if (!fs.existsSync(recoveryDir)) return { conflicts: 0, recovered: 0 };
  let conflicts = 0;
  let recovered = 0;
  const resolvedMessagesDir = path.resolve(messagesDir);
  const resolvedSourceHashesPath = path.resolve(sourceHashesPath);

  for (const directory of fs.readdirSync(recoveryDir, { withFileTypes: true })) {
    if (!directory.isDirectory()) continue;
    const transactionPath = path.join(recoveryDir, directory.name);
    const manifestPath = path.join(transactionPath, 'manifest.json');
    if (!fs.existsSync(manifestPath)) continue;

    try {
      const manifest = parseJsonWithoutDuplicateKeys(
        fs.readFileSync(manifestPath, 'utf8'),
        manifestPath
      );
      if (manifest.status !== 'in-progress') continue;
      if (!Array.isArray(manifest.entries))
        throw new TypeError('Recovery entries must be an array.');
      const files = Array.isArray(manifest.files) ? manifest.files : [];
      let transactionConflict = false;

      for (const [index, entry] of manifest.entries.entries()) {
        const pathsAreStrings = ['originalPath', 'claimPath', 'tempPath', 'rollbackPath'].every(
          (key) => typeof entry?.[key] === 'string'
        );
        const digestsAreValid = ['originalSha256', 'outputSha256'].every((key) =>
          /^[0-9a-f]{64}$/.test(entry?.[key])
        );
        if (!pathsAreStrings || !digestsAreValid) {
          transactionConflict = true;
          continue;
        }
        const resolvedOriginal = path.resolve(entry.originalPath);
        const originalIsAllowed =
          resolvedOriginal === resolvedSourceHashesPath ||
          (path.dirname(resolvedOriginal) === resolvedMessagesDir &&
            path.basename(resolvedOriginal) !== 'en.json' &&
            path.extname(resolvedOriginal) === '.json');
        const auxiliaryPathsAreSafe = ['claimPath', 'tempPath', 'rollbackPath'].every((key) =>
          pathIsInside(entry[key], transactionPath)
        );
        if (!originalIsAllowed || !auxiliaryPathsAreSafe) {
          transactionConflict = true;
          continue;
        }

        if (fs.existsSync(entry.claimPath)) {
          if (sha256(fs.readFileSync(entry.claimPath, 'utf8')) !== entry.originalSha256) {
            transactionConflict = true;
            continue;
          }
          if (fs.existsSync(entry.originalPath)) {
            if (sha256(fs.readFileSync(entry.originalPath, 'utf8')) !== entry.outputSha256) {
              transactionConflict = true;
              continue;
            }
            const displacedPath = path.join(
              transactionPath,
              `${String(index).padStart(3, '0')}-${path.basename(entry.originalPath)}.interrupted-current-${crypto.randomUUID()}`
            );
            fs.renameSync(entry.originalPath, displacedPath);
            files.push({
              kind: 'interrupted-current',
              originalPath: entry.originalPath,
              recoveryPath: displacedPath,
            });
          }
          try {
            installWithoutOverwrite(entry.claimPath, entry.originalPath);
            fsyncPath(entry.originalPath);
            fsyncPath(path.dirname(entry.originalPath));
          } catch {
            transactionConflict = true;
          }
        } else {
          if (
            !fs.existsSync(entry.originalPath) ||
            sha256(fs.readFileSync(entry.originalPath, 'utf8')) !== entry.originalSha256
          ) {
            transactionConflict = true;
          }
        }

        for (const [key, kind] of [
          ['tempPath', 'interrupted-output'],
          ['rollbackPath', 'interrupted-rollback'],
        ]) {
          if (
            fs.existsSync(entry[key]) &&
            !files.some(({ recoveryPath }) => recoveryPath === entry[key])
          ) {
            files.push({ kind, originalPath: entry.originalPath, recoveryPath: entry[key] });
          }
        }
      }

      manifest.files = files;
      manifest.status = transactionConflict ? 'conflict' : 'recovered';
      writeJsonDurably(manifestPath, manifest);
      if (transactionConflict) conflicts += 1;
      else recovered += 1;
    } catch {
      conflicts += 1;
    }
  }

  return { conflicts, recovered };
}

function recoverInterruptedTransactions(recoveryDir, paths) {
  return withRecoveryLock(recoveryDir, () =>
    recoverInterruptedTransactionsUnlocked(recoveryDir, paths)
  );
}

function commitOutputs(
  outputs,
  {
    afterClaim,
    afterFinalize,
    afterRollbackClaim,
    afterRollbackVerified,
    beforeFinalize,
    beforeReplace,
    guards = [],
    recoveryDir,
  } = {}
) {
  const token = `${process.pid}-${crypto.randomUUID()}`;
  const changedOutputs = outputs.filter(({ original, output }) => original !== output);
  if (changedOutputs.length && !recoveryDir) {
    throw new Error('A recovery directory is required for catalog updates.');
  }
  const recoveryTransaction = changedOutputs.length ? path.join(recoveryDir, token) : null;
  const staged = changedOutputs.map((entry, index) => ({
    ...entry,
    claimPath: path.join(recoveryTransaction, `${String(index).padStart(3, '0')}-original`),
    claimed: false,
    committed: false,
    rollbackPath: path.join(recoveryTransaction, `${String(index).padStart(3, '0')}-rollback`),
    tempPath: path.join(recoveryTransaction, `${String(index).padStart(3, '0')}-output`),
  }));
  const rollbackFailures = new Set();
  const recoveredFiles = [];
  let completed = false;

  function writeRecoveryManifest(status) {
    if (!recoveryTransaction) return;
    const files = recoveredFiles.filter(({ recoveryPath }) => fs.existsSync(recoveryPath));
    const entries = staged.map(
      ({ claimPath, original, output, path: originalPath, rollbackPath, tempPath }) => ({
        claimPath,
        originalPath,
        originalSha256: sha256(original),
        outputSha256: sha256(output),
        rollbackPath,
        tempPath,
      })
    );
    writeJsonDurably(path.join(recoveryTransaction, 'manifest.json'), {
      createdAt: new Date().toISOString(),
      entries,
      files,
      status,
    });
  }

  try {
    assertFilesUnchanged(guards);
    if (recoveryTransaction) {
      fs.mkdirSync(recoveryTransaction, { recursive: true });
      writeRecoveryManifest('in-progress');
      fsyncPath(recoveryDir);
    }
    for (const entry of staged) {
      const mode = fs.statSync(entry.path).mode;
      fs.writeFileSync(entry.tempPath, entry.output, { flag: 'wx', mode });
      fsyncPath(entry.tempPath);
    }

    for (const [index, entry] of staged.entries()) {
      beforeReplace?.(index, entry);
      assertFilesUnchanged(guards);
      // Moving the destination is the compare-and-swap claim. We inspect the
      // claimed inode, then install through an exclusive link that cannot
      // replace a file that reappears in the meantime.
      fs.renameSync(entry.path, entry.claimPath);
      fsyncPath(path.dirname(entry.path));
      fsyncPath(recoveryTransaction);
      entry.claimed = true;
      afterClaim?.(index, entry);
      if (fs.readFileSync(entry.claimPath, 'utf8') !== entry.original) {
        throw new Error(`Refusing to overwrite concurrently modified catalog: ${entry.path}`);
      }
      installWithoutOverwrite(entry.tempPath, entry.path);
      fsyncPath(entry.path);
      fsyncPath(path.dirname(entry.path));
      entry.committed = true;
    }
    beforeFinalize?.(staged);
    assertFilesUnchanged(guards);
    for (const entry of staged) {
      if (fs.readFileSync(entry.claimPath, 'utf8') !== entry.original) {
        throw new Error(
          `Refusing to discard concurrently modified claimed catalog: ${entry.claimPath}`
        );
      }
      if (fs.readFileSync(entry.path, 'utf8') !== entry.output) {
        throw new Error(`Installed catalog changed during synchronization: ${entry.path}`);
      }
    }
    afterFinalize?.(staged);
    for (const entry of staged) {
      recoveredFiles.push({
        kind: 'original',
        originalPath: entry.path,
        recoveryPath: entry.claimPath,
      });
    }
    writeRecoveryManifest('successful');
    completed = true;
  } catch (error) {
    const rollbackErrors = [];
    for (const [index, entry] of [...staged].reverse().entries()) {
      if (!entry.claimed) continue;
      try {
        if (!entry.committed) {
          installWithoutOverwrite(entry.claimPath, entry.path);
          entry.claimed = false;
          continue;
        }

        fs.renameSync(entry.path, entry.rollbackPath);
        fsyncPath(path.dirname(entry.path));
        fsyncPath(recoveryTransaction);
        afterRollbackClaim?.(index, entry);
        if (fs.readFileSync(entry.rollbackPath, 'utf8') !== entry.output) {
          installWithoutOverwrite(entry.rollbackPath, entry.path);
          throw new Error(
            `Rollback conflict for concurrently modified catalog: ${entry.path}. Original retained at ${entry.claimPath}`
          );
        }

        afterRollbackVerified?.(index, entry);
        installWithoutOverwrite(entry.claimPath, entry.path);
        fsyncPath(entry.path);
        fsyncPath(path.dirname(entry.path));
        entry.claimed = false;
        recoveredFiles.push({
          kind: 'rolled-back-output',
          originalPath: entry.path,
          recoveryPath: entry.rollbackPath,
        });
        entry.committed = false;
      } catch (rollbackError) {
        if (fs.existsSync(entry.claimPath)) rollbackFailures.add(entry.claimPath);
        if (fs.existsSync(entry.rollbackPath)) rollbackFailures.add(entry.rollbackPath);
        rollbackErrors.push(rollbackError);
      }
    }
    try {
      writeRecoveryManifest(rollbackErrors.length ? 'conflict' : 'rolled-back');
    } catch (manifestError) {
      rollbackErrors.push(manifestError);
    }
    if (rollbackErrors.length) {
      throw new AggregateError(
        [error, ...rollbackErrors],
        `Locale synchronization failed and rollback was incomplete. Backups retained: ${[
          ...rollbackFailures,
        ].join(', ')}`
      );
    }
    if (recoveryTransaction) {
      throw new Error(`${error.message} Rolled-back outputs retained at ${recoveryTransaction}.`, {
        cause: error,
      });
    }
    throw error;
  } finally {
    for (const entry of staged) {
      if (fs.existsSync(entry.tempPath)) fs.rmSync(entry.tempPath);
      if (completed) {
        if (fs.existsSync(entry.rollbackPath)) fs.rmSync(entry.rollbackPath);
      }
    }
    if (
      !completed &&
      recoveryTransaction &&
      !rollbackFailures.size &&
      !recoveredFiles.some(({ recoveryPath }) => fs.existsSync(recoveryPath))
    ) {
      fs.rmSync(recoveryTransaction, { recursive: true, force: true });
      if (fs.existsSync(recoveryDir) && !fs.readdirSync(recoveryDir).length) {
        fs.rmdirSync(recoveryDir);
      }
    }
  }

  return recoveryTransaction;
}

function synchronizeCatalogsLocked({
  messagesDir,
  sourceHashesPath,
  acceptSourceChanges = false,
  commitOptions,
  recoveryDir = path.join(path.dirname(sourceHashesPath), '.i18n-sync-recovery'),
}) {
  const sourcePath = path.join(messagesDir, 'en.json');
  const recoveryResult = recoverInterruptedTransactionsUnlocked(recoveryDir, {
    messagesDir,
    sourceHashesPath,
  });
  if (recoveryResult.conflicts) {
    throw new Error(
      `${recoveryResult.conflicts} interrupted i18n transaction(s) require manual recovery under ${recoveryDir}.`
    );
  }
  if (!fs.existsSync(sourceHashesPath)) {
    throw new Error(
      `Missing ${path.basename(sourceHashesPath)}. Restore the tracked source fingerprint file before synchronizing locales.`
    );
  }

  const sourceContents = fs.readFileSync(sourcePath, 'utf8');
  const sourceHashesContents = fs.readFileSync(sourceHashesPath, 'utf8');
  const source = parseJsonWithoutDuplicateKeys(sourceContents, sourcePath);
  const previousSourceHashes = parseJsonWithoutDuplicateKeys(
    sourceHashesContents,
    sourceHashesPath
  );
  validateMessageCatalog(source, sourcePath);
  validateSourceHashes(previousSourceHashes, sourceHashesPath);
  const sourceHashes = sourceHashesFor(source);
  const changedKeys = changedSourceKeys(previousSourceHashes, sourceHashes);
  const changedExistingKeys = changedExistingSourceKeys(previousSourceHashes, sourceHashes);
  const removedKeys = removedSourceKeys(previousSourceHashes, sourceHashes);
  const localePaths = fs
    .readdirSync(messagesDir)
    .filter((file) => file.endsWith('.json') && file !== 'en.json')
    .map((file) => path.join(messagesDir, file));

  // Parse every catalog before writing any of them so malformed input cannot
  // leave the locale set only partially synchronized.
  const outputs = localePaths.map((localePath) => {
    const original = fs.readFileSync(localePath, 'utf8');
    const locale = parseJsonWithoutDuplicateKeys(original, localePath);
    validateMessageCatalog(locale, localePath);
    const synchronized = synchronizeLocale(source, locale);
    return { path: localePath, original, output: `${JSON.stringify(synchronized, null, 2)}\n` };
  });

  if ((changedExistingKeys.size || removedKeys.size) && !acceptSourceChanges) {
    const changeSummary = [
      changedExistingKeys.size ? `changed: ${[...changedExistingKeys].join(', ')}` : '',
      removedKeys.size ? `removed: ${[...removedKeys].join(', ')}` : '',
    ]
      .filter(Boolean)
      .join('; ');
    throw new Error(
      `Existing source messages require review (${changeSummary}). Review these keys in every locale, then rerun pnpm i18n:sync -- --accept-source-changes.`
    );
  }

  const hashesOutput = `${JSON.stringify(sourceHashes)}\n`;
  outputs.push({
    path: sourceHashesPath,
    original: sourceHashesContents,
    output: hashesOutput,
  });
  const recoveryPath = commitOutputs(outputs, {
    ...commitOptions,
    guards: [{ path: sourcePath, original: sourceContents }],
    recoveryDir,
  });

  return {
    changedExisting: changedExistingKeys.size,
    changedOrNew: changedKeys.size,
    locales: localePaths.length,
    recoveredTransactions: recoveryResult.recovered,
    recoveryPath,
    removed: removedKeys.size,
  };
}

function synchronizeCatalogs(options) {
  const recoveryDir =
    options.recoveryDir ?? path.join(path.dirname(options.sourceHashesPath), '.i18n-sync-recovery');
  return withRecoveryLock(recoveryDir, () =>
    synchronizeCatalogsLocked({ ...options, recoveryDir })
  );
}

function main() {
  const args = process.argv.slice(2).filter((arg) => arg !== '--');
  const recoveryDir = path.join(projectDir, '.i18n-sync-recovery');
  if (args.length === 1 && args[0] === '--clean-successful-recovery') {
    const result = cleanSuccessfulRecovery(recoveryDir);
    console.log(
      `Removed ${result.removed} successful recovery transactions; preserved ${result.preserved} non-successful or incomplete entries.`
    );
    return;
  }

  const unknownArgs = args.filter((arg) => arg !== '--accept-source-changes');
  if (unknownArgs.length) {
    throw new Error(`Unknown arguments: ${unknownArgs.join(', ')}`);
  }

  const result = synchronizeCatalogs({
    messagesDir,
    recoveryDir,
    sourceHashesPath,
    acceptSourceChanges: args.includes('--accept-source-changes'),
  });
  console.log(
    `Synchronized ${result.locales} locale catalogs; recorded ${result.changedExisting} acknowledged changes, ${result.removed} removals, and ${result.changedOrNew - result.changedExisting} new messages.`
  );
  if (result.recoveryPath) {
    console.log(`Original catalogs retained for recovery at ${result.recoveryPath}.`);
  }
  if (result.recoveredTransactions) {
    console.log(`Recovered ${result.recoveredTransactions} interrupted synchronization(s).`);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  changedExistingSourceKeys,
  changedSourceKeys,
  cleanSuccessfulRecovery,
  commitOutputs,
  parseJsonWithoutDuplicateKeys,
  recoverInterruptedTransactions,
  removedSourceKeys,
  sourceHashesFor,
  synchronizeCatalogs,
  synchronizeLocale,
  validateMessageCatalog,
  validateSourceHashes,
};
