const assert = require('node:assert/strict');
const { spawn, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const {
  changedExistingSourceKeys,
  changedSourceKeys,
  cleanSuccessfulRecovery,
  parseJsonWithoutDuplicateKeys,
  recoverInterruptedTransactions,
  removedSourceKeys,
  sourceHashesFor,
  synchronizeCatalogs,
  synchronizeLocale,
  validateMessageCatalog,
  validateSourceHashes,
} = require('./i18n-sync-locales');

function writeJson(file, value) {
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function recoveryFiles(recoveryDir) {
  return fs
    .readdirSync(recoveryDir)
    .flatMap((transaction) =>
      fs
        .readdirSync(path.join(recoveryDir, transaction))
        .map((file) => path.join(recoveryDir, transaction, file))
    );
}

test('changed source messages preserve concurrently updated translations', () => {
  const previousSource = {
    changed: { defaultMessage: 'Old meaning' },
    removed: { defaultMessage: 'Removed' },
    unchanged: { defaultMessage: 'Unchanged' },
  };
  const source = {
    changed: { defaultMessage: 'New meaning' },
    added: { defaultMessage: 'Added' },
    unchanged: { defaultMessage: 'Unchanged' },
  };
  const locale = {
    changed: { defaultMessage: 'Updated translation' },
    removed: { defaultMessage: 'Old translation' },
    unchanged: { defaultMessage: 'Retained translation' },
  };
  const previousHashes = sourceHashesFor(previousSource);
  const sourceHashes = sourceHashesFor(source);

  assert.deepEqual([...changedSourceKeys(previousHashes, sourceHashes)], ['changed', 'added']);
  assert.deepEqual([...changedExistingSourceKeys(previousHashes, sourceHashes)], ['changed']);
  assert.deepEqual([...removedSourceKeys(previousHashes, sourceHashes)], ['removed']);
  const synchronized = synchronizeLocale(source, locale);
  assert.deepEqual(synchronized, {
    changed: locale.changed,
    unchanged: locale.unchanged,
    added: source.added,
  });
  assert.deepEqual(Object.keys(synchronized), ['changed', 'unchanged', 'added']);
});

test('existing source changes require acknowledgement and preserve resolved translations', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = {
    changed: { defaultMessage: 'Old meaning' },
    removed: { defaultMessage: 'Removed' },
  };
  const source = {
    changed: { defaultMessage: 'New meaning' },
    added: { defaultMessage: 'Added' },
  };
  const updatedLocale = {
    changed: { defaultMessage: 'Updated translation' },
    removed: { defaultMessage: 'Old translation' },
  };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), updatedLocale);
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));

  assert.throws(
    () => synchronizeCatalogs({ messagesDir, sourceHashesPath }),
    /Review these keys in every locale/
  );
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(messagesDir, 'de.json'))), updatedLocale);

  synchronizeCatalogs({ messagesDir, sourceHashesPath, acceptSourceChanges: true });

  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(messagesDir, 'de.json'))), {
    changed: updatedLocale.changed,
    added: source.added,
  });
  assert.deepEqual(JSON.parse(fs.readFileSync(sourceHashesPath)), sourceHashesFor(source));
});

test('malformed locale input cannot partially update catalogs or source hashes', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Old' } };
  const source = {
    existing: { defaultMessage: 'New' },
    added: { defaultMessage: 'Added' },
  };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), {
    existing: { defaultMessage: 'Bestehend' },
  });
  fs.writeFileSync(path.join(messagesDir, 'fr.json'), '{invalid');
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const deBefore = fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8');
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');

  assert.throws(
    () => synchronizeCatalogs({ messagesDir, sourceHashesPath, acceptSourceChanges: true }),
    SyntaxError
  );
  assert.equal(fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8'), deBefore);
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
});

test('source removals require acknowledgement before deleting locale entries', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = {
    retained: { defaultMessage: 'Retained' },
    removed: { defaultMessage: 'Removed' },
  };
  const source = { retained: previousSource.retained };
  const locale = {
    retained: { defaultMessage: 'Behalten' },
    removed: { defaultMessage: 'Entfernt' },
  };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), locale);
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const localeBefore = fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8');
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');

  assert.throws(() => synchronizeCatalogs({ messagesDir, sourceHashesPath }), /removed: removed/);
  assert.equal(fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8'), localeBefore);
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);

  synchronizeCatalogs({ messagesDir, sourceHashesPath, acceptSourceChanges: true });
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(messagesDir, 'de.json'))), {
    retained: locale.retained,
  });
  assert.deepEqual(JSON.parse(fs.readFileSync(sourceHashesPath)), sourceHashesFor(source));
});

test('message catalogs and source hashes reject structurally invalid JSON', () => {
  for (const invalidCatalog of [[], 'text', { key: null }, { key: { defaultMessage: 42 } }]) {
    assert.throws(() => validateMessageCatalog(invalidCatalog, 'catalog'), TypeError);
  }

  for (const invalidHashes of [[], 'text', { key: null }, { key: 'not-a-sha256-hash' }]) {
    assert.throws(() => validateSourceHashes(invalidHashes, 'hashes'), TypeError);
  }
});

test('structurally invalid locale input cannot partially update valid files', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const source = { existing: { defaultMessage: 'Existing' } };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), {
    existing: { defaultMessage: 'Bestehend' },
  });
  fs.writeFileSync(path.join(messagesDir, 'fr.json'), '[]\n');
  writeJson(sourceHashesPath, sourceHashesFor(source));
  const deBefore = fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8');
  const frBefore = fs.readFileSync(path.join(messagesDir, 'fr.json'), 'utf8');
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');

  assert.throws(
    () => synchronizeCatalogs({ messagesDir, sourceHashesPath, acceptSourceChanges: true }),
    /must be a JSON object/
  );
  assert.equal(fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8'), deBefore);
  assert.equal(fs.readFileSync(path.join(messagesDir, 'fr.json'), 'utf8'), frBefore);
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
});

test('a mid-batch replacement failure rolls back every prior catalog update', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = {
    ...previousSource,
    added: { defaultMessage: 'Added' },
  };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), {
    existing: { defaultMessage: 'Bestehend' },
  });
  writeJson(path.join(messagesDir, 'fr.json'), {
    existing: { defaultMessage: 'Existant' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const originalFiles = Object.fromEntries(
    ['de.json', 'fr.json'].map((file) => [
      file,
      fs.readFileSync(path.join(messagesDir, file), 'utf8'),
    ])
  );
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');

  assert.throws(
    () =>
      synchronizeCatalogs({
        messagesDir,
        sourceHashesPath,
        commitOptions: {
          beforeReplace(index) {
            if (index === 1) throw new Error('injected replacement failure');
          },
        },
      }),
    /injected replacement failure/
  );
  for (const [file, contents] of Object.entries(originalFiles)) {
    assert.equal(fs.readFileSync(path.join(messagesDir, file), 'utf8'), contents);
  }
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
  assert.deepEqual(fs.readdirSync(messagesDir).sort(), ['de.json', 'en.json', 'fr.json']);
  assert.equal(
    recoveryFiles(path.join(root, '.i18n-sync-recovery')).some((file) =>
      file.endsWith('-rollback')
    ),
    true
  );
});

test('a concurrent locale edit is never overwritten', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = {
    ...previousSource,
    added: { defaultMessage: 'Added' },
  };
  const concurrentLocale = {
    existing: { defaultMessage: 'Concurrent translation' },
  };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), {
    existing: { defaultMessage: 'Original translation' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');

  assert.throws(
    () =>
      synchronizeCatalogs({
        messagesDir,
        sourceHashesPath,
        commitOptions: {
          beforeReplace(index, entry) {
            if (index === 0) writeJson(entry.path, concurrentLocale);
          },
        },
      }),
    /concurrently modified catalog/
  );
  assert.deepEqual(
    JSON.parse(fs.readFileSync(path.join(messagesDir, 'de.json'))),
    concurrentLocale
  );
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
  assert.deepEqual(fs.readdirSync(messagesDir).sort(), ['de.json', 'en.json']);
  assert.deepEqual(fs.readdirSync(root).sort(), ['messages', 'source-hashes.json']);
});

test('rollback preserves a concurrent edit to an already replaced catalog', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = {
    ...previousSource,
    added: { defaultMessage: 'Added' },
  };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), {
    existing: { defaultMessage: 'Bestehend' },
  });
  writeJson(path.join(messagesDir, 'fr.json'), {
    existing: { defaultMessage: 'Existant' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const originalFiles = Object.fromEntries(
    ['de.json', 'fr.json'].map((file) => [
      file,
      fs.readFileSync(path.join(messagesDir, file), 'utf8'),
    ])
  );
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');
  const concurrentLocale = `${JSON.stringify(
    { existing: { defaultMessage: 'Concurrent translation' } },
    null,
    2
  )}\n`;
  let committedPath;

  assert.throws(
    () =>
      synchronizeCatalogs({
        messagesDir,
        sourceHashesPath,
        commitOptions: {
          beforeReplace(index, entry) {
            if (index === 0) {
              committedPath = entry.path;
            } else if (index === 1) {
              fs.writeFileSync(committedPath, concurrentLocale);
              throw new Error('injected later failure');
            }
          },
        },
      }),
    /rollback was incomplete/
  );

  assert.equal(fs.readFileSync(committedPath, 'utf8'), concurrentLocale);
  const committedFile = path.basename(committedPath);
  const untouchedFile = committedFile === 'de.json' ? 'fr.json' : 'de.json';
  assert.equal(
    fs.readFileSync(path.join(messagesDir, untouchedFile), 'utf8'),
    originalFiles[untouchedFile]
  );
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
  const backups = recoveryFiles(path.join(root, '.i18n-sync-recovery')).filter((file) =>
    file.endsWith('-original')
  );
  assert.equal(backups.length, 1);
  assert.equal(fs.readFileSync(backups[0], 'utf8'), originalFiles[committedFile]);
});

test('claim protocol never overwrites a destination that reappears before install', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = { ...previousSource, added: { defaultMessage: 'Added' } };
  const concurrentLocale = `${JSON.stringify(
    { existing: { defaultMessage: 'Concurrent translation' } },
    null,
    2
  )}\n`;
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), {
    existing: { defaultMessage: 'Original translation' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');

  assert.throws(
    () =>
      synchronizeCatalogs({
        messagesDir,
        sourceHashesPath,
        commitOptions: {
          afterClaim(index, entry) {
            if (index === 0) fs.writeFileSync(entry.path, concurrentLocale);
          },
        },
      }),
    /rollback was incomplete/
  );
  assert.equal(fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8'), concurrentLocale);
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
  assert.equal(
    recoveryFiles(path.join(root, '.i18n-sync-recovery')).filter((file) =>
      file.endsWith('-original')
    ).length,
    1
  );
});

test('rollback never overwrites a destination that reappears before restoration', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = { ...previousSource, added: { defaultMessage: 'Added' } };
  const concurrentLocale = `${JSON.stringify(
    { existing: { defaultMessage: 'Concurrent rollback translation' } },
    null,
    2
  )}\n`;
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), {
    existing: { defaultMessage: 'Bestehend' },
  });
  writeJson(path.join(messagesDir, 'fr.json'), {
    existing: { defaultMessage: 'Existant' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');

  assert.throws(
    () =>
      synchronizeCatalogs({
        messagesDir,
        sourceHashesPath,
        commitOptions: {
          beforeReplace(index) {
            if (index === 1) throw new Error('injected later failure');
          },
          afterRollbackClaim(_index, entry) {
            fs.writeFileSync(entry.path, concurrentLocale);
          },
        },
      }),
    /rollback was incomplete/
  );
  assert.equal(fs.readFileSync(path.join(messagesDir, 'de.json'), 'utf8'), concurrentLocale);
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
  const retainedFiles = recoveryFiles(path.join(root, '.i18n-sync-recovery')).filter(
    (file) => file.endsWith('-original') || file.endsWith('-rollback')
  );
  assert.equal(retainedFiles.length, 2);
});

test('locale validator exits nonzero for a structurally invalid catalog', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-validator-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const scriptsDir = path.join(root, 'scripts');
  const messagesDir = path.join(root, 'src', 'i18n', 'messages');
  fs.mkdirSync(scriptsDir, { recursive: true });
  fs.mkdirSync(messagesDir, { recursive: true });
  fs.copyFileSync(
    path.join(__dirname, 'i18n-validate-locale.js'),
    path.join(scriptsDir, 'i18n-validate-locale.js')
  );
  fs.copyFileSync(
    path.join(__dirname, 'i18n-sync-locales.js'),
    path.join(scriptsDir, 'i18n-sync-locales.js')
  );

  const source = { key: { defaultMessage: 'Source message' } };
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(path.join(messagesDir, 'de.json'), { key: null });
  writeJson(path.join(scriptsDir, 'i18n-source-hashes.json'), sourceHashesFor(source));

  const result = spawnSync(
    process.execPath,
    [path.join(scriptsDir, 'i18n-validate-locale.js'), 'de'],
    {
      encoding: 'utf8',
      env: {
        ...process.env,
        NODE_PATH: [path.resolve(__dirname, '..', '..', 'node_modules'), process.env.NODE_PATH]
          .filter(Boolean)
          .join(path.delimiter),
      },
    }
  );

  assert.equal(result.status, 1);
  assert.match(result.stderr, /message "key" must be a JSON object/);
});

test('duplicate JSON keys are rejected even when their spellings use escapes', () => {
  assert.throws(
    () =>
      parseJsonWithoutDuplicateKeys(
        '{"key":{"defaultMessage":"one"},"\\u006bey":{"defaultMessage":"two"}}',
        'catalog'
      ),
    /duplicate JSON key "key"/
  );
  assert.throws(
    () =>
      parseJsonWithoutDuplicateKeys(
        '{"key":{"defaultMessage":"one","defaultMessage":"two"}}',
        'catalog'
      ),
    /duplicate JSON key "defaultMessage"/
  );
});

test('duplicate keys in source, locale, or hash inputs cannot mutate any file', (t) => {
  const scenarios = [
    {
      file: 'source',
      contents: '{"key":{"defaultMessage":"one"},"key":{"defaultMessage":"two"}}\n',
    },
    {
      file: 'locale',
      contents: '{"key":{"defaultMessage":"eins"},"key":{"defaultMessage":"zwei"}}\n',
    },
    {
      file: 'hashes',
      contents: `{"key":"${'a'.repeat(64)}","key":"${'b'.repeat(64)}"}\n`,
    },
  ];

  for (const scenario of scenarios) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-duplicate-'));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const messagesDir = path.join(root, 'messages');
    const sourceHashesPath = path.join(root, 'source-hashes.json');
    const sourcePath = path.join(messagesDir, 'en.json');
    const localePath = path.join(messagesDir, 'de.json');
    fs.mkdirSync(messagesDir);

    const source = { key: { defaultMessage: 'Source' } };
    writeJson(sourcePath, source);
    writeJson(localePath, { key: { defaultMessage: 'Quelle' } });
    writeJson(sourceHashesPath, sourceHashesFor(source));
    const scenarioPath =
      scenario.file === 'source'
        ? sourcePath
        : scenario.file === 'locale'
          ? localePath
          : sourceHashesPath;
    fs.writeFileSync(scenarioPath, scenario.contents);
    const before = Object.fromEntries(
      [sourcePath, localePath, sourceHashesPath].map((file) => [
        file,
        fs.readFileSync(file, 'utf8'),
      ])
    );

    assert.throws(
      () => synchronizeCatalogs({ messagesDir, sourceHashesPath, acceptSourceChanges: true }),
      /duplicate JSON key "key"/
    );
    for (const [file, contents] of Object.entries(before)) {
      assert.equal(fs.readFileSync(file, 'utf8'), contents);
    }
  }
});

test('a write through a pre-claim file descriptor is restored instead of discarded', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = { ...previousSource, added: { defaultMessage: 'Added' } };
  const localePath = path.join(messagesDir, 'de.json');
  const concurrentLocale = `${JSON.stringify(
    { existing: { defaultMessage: 'Descriptor translation' } },
    null,
    2
  )}\n`;
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(localePath, {
    existing: { defaultMessage: 'Original translation' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');
  let descriptor;

  try {
    assert.throws(
      () =>
        synchronizeCatalogs({
          messagesDir,
          sourceHashesPath,
          commitOptions: {
            beforeReplace(index, entry) {
              if (index === 0) descriptor = fs.openSync(entry.path, 'r+');
            },
            beforeFinalize() {
              fs.ftruncateSync(descriptor, 0);
              fs.writeSync(descriptor, concurrentLocale, 0, 'utf8');
            },
          },
        }),
      /concurrently modified claimed catalog/
    );
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
  }

  assert.equal(fs.readFileSync(localePath, 'utf8'), concurrentLocale);
  assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
  assert.deepEqual(fs.readdirSync(messagesDir).sort(), ['de.json', 'en.json']);
  assert.equal(
    recoveryFiles(path.join(root, '.i18n-sync-recovery')).some((file) =>
      file.endsWith('-rollback')
    ),
    true
  );
});

test('a stale-descriptor write after final verification remains recoverable', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const recoveryDir = path.join(root, 'recovery');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = { ...previousSource, added: { defaultMessage: 'Added' } };
  const localePath = path.join(messagesDir, 'de.json');
  const concurrentLocale = `${JSON.stringify(
    { existing: { defaultMessage: 'Late descriptor translation' } },
    null,
    2
  )}\n`;
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(localePath, {
    existing: { defaultMessage: 'Original translation' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  let descriptor;
  let result;

  try {
    result = synchronizeCatalogs({
      messagesDir,
      recoveryDir,
      sourceHashesPath,
      commitOptions: {
        beforeReplace(index, entry) {
          if (index === 0) descriptor = fs.openSync(entry.path, 'r+');
        },
        afterFinalize() {
          fs.ftruncateSync(descriptor, 0);
          fs.writeSync(descriptor, concurrentLocale, 0, 'utf8');
        },
      },
    });
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
  }

  assert.equal(JSON.parse(fs.readFileSync(localePath)).added.defaultMessage, 'Added');
  assert.equal(result.recoveryPath.startsWith(recoveryDir), true);
  const manifest = JSON.parse(
    fs.readFileSync(path.join(result.recoveryPath, 'manifest.json'), 'utf8')
  );
  const recoveredLocale = manifest.files.find(
    ({ kind, originalPath }) => kind === 'original' && originalPath === localePath
  );
  assert.equal(fs.readFileSync(recoveredLocale.recoveryPath, 'utf8'), concurrentLocale);
  assert.equal(
    manifest.files.some(({ originalPath }) => originalPath === localePath),
    true
  );
  assert.equal(manifest.status, 'successful');
});

test('a stale-descriptor write during rollback remains recoverable', (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-sync-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const messagesDir = path.join(root, 'messages');
  const recoveryDir = path.join(root, 'recovery');
  const sourceHashesPath = path.join(root, 'source-hashes.json');
  fs.mkdirSync(messagesDir);

  const previousSource = { existing: { defaultMessage: 'Existing' } };
  const source = { ...previousSource, added: { defaultMessage: 'Added' } };
  const localePath = path.join(messagesDir, 'de.json');
  const originalLocale = { existing: { defaultMessage: 'Original translation' } };
  const concurrentOutput = `${JSON.stringify(
    { existing: originalLocale.existing, added: { defaultMessage: 'Late rollback write' } },
    null,
    2
  )}\n`;
  writeJson(path.join(messagesDir, 'en.json'), source);
  writeJson(localePath, originalLocale);
  writeJson(path.join(messagesDir, 'fr.json'), {
    existing: { defaultMessage: 'Existant' },
  });
  writeJson(sourceHashesPath, sourceHashesFor(previousSource));
  let descriptor;

  try {
    assert.throws(
      () =>
        synchronizeCatalogs({
          messagesDir,
          recoveryDir,
          sourceHashesPath,
          commitOptions: {
            beforeReplace(index) {
              if (index === 1) {
                descriptor = fs.openSync(localePath, 'r+');
                throw new Error('injected later failure');
              }
            },
            afterRollbackVerified() {
              fs.ftruncateSync(descriptor, 0);
              fs.writeSync(descriptor, concurrentOutput, 0, 'utf8');
            },
          },
        }),
      /Rolled-back outputs retained/
    );
  } finally {
    if (descriptor !== undefined) fs.closeSync(descriptor);
  }

  assert.deepEqual(JSON.parse(fs.readFileSync(localePath)), originalLocale);
  const transactions = fs.readdirSync(recoveryDir);
  assert.equal(transactions.length, 1);
  const transactionPath = path.join(recoveryDir, transactions[0]);
  const manifest = JSON.parse(fs.readFileSync(path.join(transactionPath, 'manifest.json'), 'utf8'));
  const rolledBackOutput = manifest.files.find(
    ({ kind, originalPath }) => kind === 'rolled-back-output' && originalPath === localePath
  );
  assert.equal(fs.readFileSync(rolledBackOutput.recoveryPath, 'utf8'), concurrentOutput);
  assert.equal(
    manifest.files.some(({ kind }) => kind === 'rolled-back-output'),
    true
  );
  assert.equal(manifest.status, 'rolled-back');
});

test('recovery cleanup removes only successful transactions', (t) => {
  const recoveryDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-recovery-'));
  t.after(() => fs.rmSync(recoveryDir, { recursive: true, force: true }));
  for (const status of ['successful', 'rolled-back', 'conflict']) {
    const transaction = path.join(recoveryDir, status);
    fs.mkdirSync(transaction);
    writeJson(path.join(transaction, 'manifest.json'), { files: [], status });
  }
  fs.mkdirSync(path.join(recoveryDir, 'incomplete'));
  fs.writeFileSync(path.join(recoveryDir, 'unexpected-file'), 'preserve');

  const result = cleanSuccessfulRecovery(recoveryDir);

  assert.deepEqual(result, { preserved: 4, removed: 1 });
  assert.equal(fs.existsSync(path.join(recoveryDir, 'successful')), false);
  for (const entry of ['rolled-back', 'conflict', 'incomplete', 'unexpected-file']) {
    assert.equal(fs.existsSync(path.join(recoveryDir, entry)), true);
  }
});

test(
  'the next invocation recovers transactions interrupted by process termination',
  { skip: process.platform === 'win32' },
  (t) => {
    for (const phase of ['afterClaim', 'beforeFinalize']) {
      const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-crash-'));
      t.after(() => fs.rmSync(root, { recursive: true, force: true }));
      const messagesDir = path.join(root, 'messages');
      const recoveryDir = path.join(root, 'recovery');
      const sourceHashesPath = path.join(root, 'source-hashes.json');
      fs.mkdirSync(messagesDir);

      const previousSource = { existing: { defaultMessage: 'Existing' } };
      const source = { ...previousSource, added: { defaultMessage: 'Added' } };
      const localePath = path.join(messagesDir, 'de.json');
      writeJson(path.join(messagesDir, 'en.json'), source);
      writeJson(localePath, {
        existing: { defaultMessage: 'Original translation' },
      });
      writeJson(sourceHashesPath, sourceHashesFor(previousSource));
      const localeBefore = fs.readFileSync(localePath, 'utf8');
      const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');
      const modulePath = path.join(__dirname, 'i18n-sync-locales.js');
      const childScript = `
        const { synchronizeCatalogs } = require(${JSON.stringify(modulePath)});
        const commitOptions = {};
        commitOptions[${JSON.stringify(phase)}] = () => process.kill(process.pid, 'SIGKILL');
        synchronizeCatalogs({
          messagesDir: ${JSON.stringify(messagesDir)},
          recoveryDir: ${JSON.stringify(recoveryDir)},
          sourceHashesPath: ${JSON.stringify(sourceHashesPath)},
          commitOptions,
        });
      `;

      const child = spawnSync(process.execPath, ['-e', childScript], { encoding: 'utf8' });
      assert.equal(child.signal, 'SIGKILL');
      const transactions = fs
        .readdirSync(recoveryDir)
        .filter((entry) => entry !== '.i18n-sync.lock');
      assert.equal(transactions.length, 1);
      const manifestPath = path.join(recoveryDir, transactions[0], 'manifest.json');
      assert.equal(JSON.parse(fs.readFileSync(manifestPath)).status, 'in-progress');

      assert.deepEqual(
        recoverInterruptedTransactions(recoveryDir, { messagesDir, sourceHashesPath }),
        { conflicts: 0, recovered: 1 }
      );
      assert.equal(fs.readFileSync(localePath, 'utf8'), localeBefore);
      assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
      assert.equal(JSON.parse(fs.readFileSync(manifestPath)).status, 'recovered');
    }
  }
);

test(
  'simultaneous recovery attempts are serialized by the process lock',
  { skip: process.platform === 'win32' },
  async (t) => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-lock-'));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const messagesDir = path.join(root, 'messages');
    const recoveryDir = path.join(root, 'recovery');
    const sourceHashesPath = path.join(root, 'source-hashes.json');
    fs.mkdirSync(messagesDir);

    const previousSource = { existing: { defaultMessage: 'Existing' } };
    const source = { ...previousSource, added: { defaultMessage: 'Added' } };
    const localePath = path.join(messagesDir, 'de.json');
    writeJson(path.join(messagesDir, 'en.json'), source);
    writeJson(localePath, {
      existing: { defaultMessage: 'Original translation' },
    });
    writeJson(sourceHashesPath, sourceHashesFor(previousSource));
    const localeBefore = fs.readFileSync(localePath, 'utf8');
    const hashesBefore = fs.readFileSync(sourceHashesPath, 'utf8');
    const modulePath = path.join(__dirname, 'i18n-sync-locales.js');
    const seedScript = `
      const { synchronizeCatalogs } = require(${JSON.stringify(modulePath)});
      synchronizeCatalogs({
        messagesDir: ${JSON.stringify(messagesDir)},
        recoveryDir: ${JSON.stringify(recoveryDir)},
        sourceHashesPath: ${JSON.stringify(sourceHashesPath)},
        commitOptions: { afterClaim: () => process.kill(process.pid, 'SIGKILL') },
      });
    `;
    const seed = spawnSync(process.execPath, ['-e', seedScript]);
    assert.equal(seed.signal, 'SIGKILL');

    const recoverScript = `
      const { recoverInterruptedTransactions } = require(${JSON.stringify(modulePath)});
      try {
        console.log(JSON.stringify(recoverInterruptedTransactions(
          ${JSON.stringify(recoveryDir)},
          { messagesDir: ${JSON.stringify(messagesDir)}, sourceHashesPath: ${JSON.stringify(sourceHashesPath)} }
        )));
      } catch (error) {
        console.error(error.message);
        process.exitCode = 2;
      }
    `;
    const children = [0, 1].map(() =>
      spawn(process.execPath, ['-e', recoverScript], { stdio: ['ignore', 'pipe', 'pipe'] })
    );
    const results = await Promise.all(
      children.map(
        (child) =>
          new Promise((resolve) => {
            let stderr = '';
            let stdout = '';
            child.stderr.on('data', (chunk) => (stderr += chunk));
            child.stdout.on('data', (chunk) => (stdout += chunk));
            child.on('close', (code) => resolve({ code, stderr, stdout }));
          })
      )
    );

    assert.equal(
      results.some(({ code }) => code === 0),
      true
    );
    for (const result of results.filter(({ code }) => code !== 0)) {
      assert.match(result.stderr, /Another i18n synchronization is running/);
    }
    assert.equal(fs.readFileSync(localePath, 'utf8'), localeBefore);
    assert.equal(fs.readFileSync(sourceHashesPath, 'utf8'), hashesBefore);
    const transaction = fs.readdirSync(recoveryDir).find((entry) => entry !== '.i18n-sync.lock');
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(recoveryDir, transaction, 'manifest.json'))).status,
      'recovered'
    );
  }
);

test(
  'interrupted recovery blocks instead of displacing a newer destination edit',
  { skip: process.platform === 'win32' },
  (t) => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-i18n-conflict-'));
    t.after(() => fs.rmSync(root, { recursive: true, force: true }));
    const messagesDir = path.join(root, 'messages');
    const recoveryDir = path.join(root, 'recovery');
    const sourceHashesPath = path.join(root, 'source-hashes.json');
    fs.mkdirSync(messagesDir);

    const previousSource = { existing: { defaultMessage: 'Existing' } };
    const source = { ...previousSource, added: { defaultMessage: 'Added' } };
    const localePath = path.join(messagesDir, 'de.json');
    const originalLocale = `${JSON.stringify(
      { existing: { defaultMessage: 'Original translation' } },
      null,
      2
    )}\n`;
    const newerLocale = `${JSON.stringify(
      { existing: { defaultMessage: 'Newer translator edit' } },
      null,
      2
    )}\n`;
    writeJson(path.join(messagesDir, 'en.json'), source);
    fs.writeFileSync(localePath, originalLocale);
    writeJson(sourceHashesPath, sourceHashesFor(previousSource));
    const modulePath = path.join(__dirname, 'i18n-sync-locales.js');
    const seedScript = `
      const { synchronizeCatalogs } = require(${JSON.stringify(modulePath)});
      synchronizeCatalogs({
        messagesDir: ${JSON.stringify(messagesDir)},
        recoveryDir: ${JSON.stringify(recoveryDir)},
        sourceHashesPath: ${JSON.stringify(sourceHashesPath)},
        commitOptions: { beforeFinalize: () => process.kill(process.pid, 'SIGKILL') },
      });
    `;
    const seed = spawnSync(process.execPath, ['-e', seedScript]);
    assert.equal(seed.signal, 'SIGKILL');
    fs.writeFileSync(localePath, newerLocale);

    assert.throws(
      () => synchronizeCatalogs({ messagesDir, recoveryDir, sourceHashesPath }),
      /require manual recovery/
    );
    assert.equal(fs.readFileSync(localePath, 'utf8'), newerLocale);
    const transaction = fs.readdirSync(recoveryDir).find((entry) => entry !== '.i18n-sync.lock');
    const manifest = JSON.parse(
      fs.readFileSync(path.join(recoveryDir, transaction, 'manifest.json'), 'utf8')
    );
    assert.equal(manifest.status, 'conflict');
    const localeEntry = manifest.entries.find(({ originalPath }) => originalPath === localePath);
    assert.equal(fs.readFileSync(localeEntry.claimPath, 'utf8'), originalLocale);
  }
);
