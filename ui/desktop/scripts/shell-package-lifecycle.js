#!/usr/bin/env node

const fs = require('node:fs');
const http = require('node:http');
const net = require('node:net');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { chromium } = require('@playwright/test');
const { packageLayout } = require('./shell-package-verifier');
const { resolveConsumerManifest } = require('./shell-consumer');

const MAX_LOG_BYTES = 64 * 1024;
const STARTUP_TIMEOUT_MS = 60_000;
const STATE_TIMEOUT_MS = 30_000;
const EXIT_TIMEOUT_MS = 15_000;

function fail(message) {
  throw new Error(message);
}

function exactKeys(value, keys, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    fail(`${label} must be an object`);
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    fail(`${label} has unknown or missing fields`);
  }
}

function option(args, name, fallback) {
  const indexes = args.flatMap((entry, index) => (entry === name ? [index] : []));
  if (indexes.length === 0) return fallback;
  if (indexes.length > 1) fail(`${name} may be provided only once`);
  const index = indexes[0];
  const value = args[index + 1];
  if (!value || value.startsWith('--')) fail(`${name} requires a value`);
  args.splice(index, 2);
  return value;
}

function boundedAppend(current, chunk) {
  const next = `${current}${chunk}`;
  return next.length <= MAX_LOG_BYTES ? next : next.slice(next.length - MAX_LOG_BYTES);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function availablePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on('error', reject);
    server.listen({ host: '127.0.0.1', port: 0 }, () => {
      const address = server.address();
      if (!address || typeof address === 'string') {
        server.close();
        reject(new Error('could not allocate a loopback debugging port'));
        return;
      }
      server.close((error) => (error ? reject(error) : resolve(address.port)));
    });
  });
}

function isolatedEnvironment(root) {
  fs.mkdirSync(root, { recursive: true, mode: 0o700 });
  return {
    ...process.env,
    GOSLING_DISABLE_KEYRING: '1',
    GOSLING_PATH_ROOT: path.join(root, 'gosling'),
    GOSLING_PLAYWRIGHT_USER_DATA_DIR: root,
    ELECTRON_DISABLE_SECURITY_WARNINGS: 'true',
    ENABLE_PLAYWRIGHT: 'true',
  };
}

function waitForExit(child, timeout = EXIT_TIMEOUT_MS) {
  if (child.exitCode !== null) return Promise.resolve(child.exitCode);
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error('packaged shell did not exit after close')),
      timeout
    );
    child.once('exit', (code) => {
      clearTimeout(timer);
      resolve(code);
    });
  });
}

async function terminateChild(child) {
  if (child.exitCode !== null) return;
  child.kill('SIGTERM');
  try {
    await waitForExit(child, 2_000);
  } catch {
    if (child.exitCode === null) child.kill('SIGKILL');
    await waitForExit(child, 2_000).catch(() => {});
  }
}

function debuggerEndpoint(port) {
  return new Promise((resolve, reject) => {
    const request = http.get(
      { host: '127.0.0.1', port, path: '/json/version', timeout: 500 },
      (response) => {
        let body = '';
        response.setEncoding('utf8');
        response.on('data', (chunk) => {
          body = boundedAppend(body, chunk);
        });
        response.on('end', () => {
          try {
            const endpoint = JSON.parse(body).webSocketDebuggerUrl;
            if (typeof endpoint !== 'string' || !endpoint.startsWith(`ws://127.0.0.1:${port}/`)) {
              reject(new Error('CDP metadata did not contain the expected loopback endpoint'));
              return;
            }
            resolve(endpoint);
          } catch {
            reject(new Error('CDP metadata was malformed'));
          }
        });
      }
    );
    request.on('timeout', () => request.destroy(new Error('CDP metadata request timed out')));
    request.on('error', reject);
  });
}

async function connectBrowser(port, child, logs) {
  const deadline = Date.now() + STARTUP_TIMEOUT_MS;
  let lastError = 'CDP endpoint unavailable';
  while (Date.now() < deadline) {
    if (child.spawnError) fail(`packaged shell could not launch: ${child.spawnError}; ${logs()}`);
    if (child.exitCode !== null) {
      fail(`packaged shell exited before CDP startup (${child.exitCode}): ${logs()}`);
    }
    try {
      const endpoint = await debuggerEndpoint(port);
      return await chromium.connectOverCDP(endpoint, { timeout: 10_000 });
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
      await delay(100);
    }
  }
  fail(`packaged shell CDP startup timed out: ${lastError}; ${logs()}`);
}

async function firstPage(browser) {
  const deadline = Date.now() + STATE_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const page = browser.contexts().flatMap((context) => context.pages())[0];
    if (page) return page;
    await delay(50);
  }
  fail('packaged shell created no renderer page');
}

async function waitForState(page, expected) {
  const deadline = Date.now() + STATE_TIMEOUT_MS;
  let last;
  while (Date.now() < deadline) {
    try {
      last = await page.evaluate(async () => window.goslingShell.runtime.read());
    } catch {
      await delay(100);
      continue;
    }
    if (last.lifecycleState === expected) return last;
    if (
      ['degraded', 'relink_required', 'incompatible', 'offline', 'stopped', 'fatal'].includes(
        last.lifecycleState
      )
    ) {
      fail(`packaged shell reached terminal ${last.lifecycleState}: ${JSON.stringify(last)}`);
    }
    await delay(100);
  }
  fail(`packaged shell did not reach ${expected}: ${JSON.stringify(last)}`);
}

async function launchPackagedShell(input) {
  const debugPort = await availablePort();
  const output = { stdout: '', stderr: '' };
  const child = spawn(input.layout.appExecutable, [`--remote-debugging-port=${debugPort}`], {
    cwd: input.workingDirectory,
    env: isolatedEnvironment(input.root),
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.spawnError = undefined;
  child.on('error', (error) => {
    child.spawnError = error.message;
  });
  child.stdout?.on('data', (chunk) => {
    output.stdout = boundedAppend(output.stdout, chunk.toString());
  });
  child.stderr?.on('data', (chunk) => {
    output.stderr = boundedAppend(output.stderr, chunk.toString());
  });
  const logs = () => `stdout=${output.stdout} stderr=${output.stderr}`;
  let browser;
  try {
    browser = await connectBrowser(debugPort, child, logs);
    const page = await firstPage(browser);
    await page.waitForLoadState('domcontentloaded', { timeout: STATE_TIMEOUT_MS });
    return {
      root: input.root,
      productId: input.productId,
      waitForState: (state) => waitForState(page, state),
      read: () => page.evaluate(async () => window.goslingShell.runtime.read()),
      stop: (generation) =>
        page.evaluate(
          async (value) => window.goslingShell.runtime.stop({ generation: value }),
          generation
        ),
      retry: (generation) =>
        page.evaluate(
          async (value) => window.goslingShell.runtime.retry({ generation: value }),
          generation
        ),
      interruptBackend: () =>
        interruptRegisteredBackend(input.root, input.productId, child.pid, input.layout.binary),
      async close() {
        await page.close();
        const code = await waitForExit(child);
        await browser.close().catch(() => {});
        if (code !== 0) fail(`packaged shell exited with ${code}: ${logs()}`);
      },
      async terminate() {
        await browser?.close().catch(() => {});
        await terminateChild(child);
      },
    };
  } catch (error) {
    await browser?.close().catch(() => {});
    await terminateChild(child);
    throw error;
  }
}

function registryFiles(root) {
  const found = [];
  const pending = [root];
  let visited = 0;
  while (pending.length > 0) {
    const directory = pending.pop();
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      visited += 1;
      if (visited > 10_000) fail('isolated lifecycle root exceeded the file inspection bound');
      const candidate = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) continue;
      if (entry.isDirectory()) pending.push(candidate);
      if (entry.isFile() && entry.name === 'backend-processes.json') found.push(candidate);
    }
  }
  return found.sort();
}

function backendRegistry(root, productId) {
  return path.join(root, 'app-data', productId, 'backend-processes.json');
}

function interruptRegisteredBackend(root, productId, parentPid, binary) {
  const registry = backendRegistry(root, productId);
  let value;
  try {
    value = JSON.parse(fs.readFileSync(registry, 'utf8'));
  } catch {
    fail('packaged shell backend registry is unavailable for failure injection');
  }
  if (value.version !== 1 || !Array.isArray(value.processes) || value.processes.length !== 1) {
    fail('packaged shell backend registry is not singular for failure injection');
  }
  const [record] = value.processes;
  if (
    !Number.isSafeInteger(record.pid) ||
    record.pid < 1 ||
    record.parentPid !== parentPid ||
    path.resolve(record.binaryPath) !== path.resolve(binary)
  ) {
    fail('packaged shell backend registry does not identify the launched package');
  }
  process.kill(record.pid, 'SIGTERM');
  return record.pid;
}

function assertEmptyRegistries(root) {
  const files = registryFiles(root);
  if (files.length !== 1) fail('packaged shell did not leave exactly one process registry');
  for (const file of files) {
    let value;
    try {
      value = JSON.parse(fs.readFileSync(file, 'utf8'));
    } catch {
      fail('packaged shell process registry is malformed after close');
    }
    if (value.version !== 1 || !Array.isArray(value.processes) || value.processes.length !== 0) {
      fail('packaged shell left a backend process registered after close');
    }
  }
  return files.length;
}

function resolveApplication(input) {
  const consumer = resolveConsumerManifest(path.resolve(input.consumerFile));
  if (consumer.profile.profilePath !== path.resolve(input.profileFile)) {
    fail('lifecycle profile and consumer select different products');
  }
  const profile = consumer.profile.profile;
  return {
    consumer,
    profile,
    layout: packageLayout(
      profile,
      input.platform,
      input.architecture,
      path.resolve(input.packageDirectory)
    ),
  };
}

async function runPackageLifecycle(input, dependencies = {}) {
  const launch = dependencies.launch ?? launchPackagedShell;
  const inspectRegistries = dependencies.inspectRegistries ?? assertEmptyRegistries;
  const application = resolveApplication(input);
  const workingDirectory = fs.realpathSync(input.workingDirectory ?? process.cwd());
  const root = fs.mkdtempSync(
    path.join(os.tmpdir(), `${application.profile.product.id}-lifecycle-`)
  );
  const observations = [];
  let active;
  try {
    for (let cycle = 1; cycle <= 2; cycle += 1) {
      const cycleRoot = path.join(root, `cycle-${cycle}`);
      fs.mkdirSync(cycleRoot, { recursive: true, mode: 0o700 });
      active = await launch({
        layout: application.layout,
        productId: application.profile.product.id,
        root: cycleRoot,
        workingDirectory,
      });
      const ready = await active.waitForState('ready');
      const observation = { cycle, readyGeneration: ready.generation };
      if (cycle === 1) {
        observation.interruptedBackendPid = active.interruptBackend();
        const offline = await active.waitForState('offline');
        const retry = await active.retry(offline.generation);
        if (!retry.accepted) fail('packaged shell rejected retry after backend exit');
        const restarted = await active.waitForState('ready');
        observation.retryGeneration = restarted.generation;
      } else {
        const stop = await active.stop(ready.generation);
        if (!stop.accepted) fail('packaged shell rejected explicit stop');
        await active.waitForState('stopped');
      }
      await active.close();
      active = undefined;
      observation.registryFiles = inspectRegistries(cycleRoot);
      observations.push(observation);
    }
    return {
      schemaVersion: 1,
      kind: 'lifecycle',
      productId: application.profile.product.id,
      target: input.target,
      observations,
      passed: true,
    };
  } finally {
    await active?.terminate();
    fs.rmSync(root, { recursive: true, force: true });
  }
}

async function runPackageCoexistence(inputs, dependencies = {}) {
  if (!Array.isArray(inputs) || inputs.length < 2) fail('coexistence requires at least two shells');
  const launch = dependencies.launch ?? launchPackagedShell;
  const inspectRegistries = dependencies.inspectRegistries ?? assertEmptyRegistries;
  const applications = inputs.map(resolveApplication);
  const productIds = applications.map((entry) => entry.profile.product.id);
  if (new Set(productIds).size !== productIds.length) fail('coexistence products must be distinct');
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'gosling-shell-coexistence-'));
  const active = [];
  try {
    for (let index = 0; index < applications.length; index += 1) {
      const appRoot = path.join(root, `app-${index}`);
      fs.mkdirSync(appRoot, { recursive: true, mode: 0o700 });
      active.push(
        await launch({
          layout: applications[index].layout,
          productId: productIds[index],
          root: appRoot,
          workingDirectory: fs.realpathSync(inputs[index].workingDirectory ?? process.cwd()),
        })
      );
    }
    const ready = await Promise.all(active.map((entry) => entry.waitForState('ready')));
    await active[0].close();
    const survivor = await active[1].read();
    if (survivor.lifecycleState !== 'ready') fail('closing one shell disturbed a coexisting shell');
    for (let index = 1; index < active.length; index += 1) await active[index].close();
    const registries = active.map((entry) => inspectRegistries(entry.root));
    active.length = 0;
    return {
      schemaVersion: 1,
      kind: 'coexistence',
      productIds,
      readyGenerations: ready.map((entry) => entry.generation),
      registryFiles: registries,
      passed: true,
    };
  } finally {
    await Promise.all(active.map((entry) => entry.terminate()));
    fs.rmSync(root, { recursive: true, force: true });
  }
}

function readCoexistence(file, platform, architecture) {
  const document = JSON.parse(fs.readFileSync(file, 'utf8'));
  exactKeys(document, ['schemaVersion', 'applications'], 'coexistence input');
  if (document.schemaVersion !== 1 || !Array.isArray(document.applications)) {
    fail('coexistence input schema is invalid');
  }
  return document.applications.map((entry, index) => {
    exactKeys(entry, ['profileFile', 'consumerFile', 'packageDirectory'], `applications[${index}]`);
    return { ...entry, platform, architecture };
  });
}

function writeReport(file, report) {
  if (!file) return;
  const resolved = path.resolve(file);
  fs.mkdirSync(path.dirname(resolved), { recursive: true });
  const temporary = `${resolved}.${process.pid}.tmp`;
  fs.writeFileSync(temporary, `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 });
  fs.renameSync(temporary, resolved);
}

function usage() {
  return [
    'Usage:',
    '  node scripts/shell-package-lifecycle.js lifecycle --profile <file> --consumer <file> --package <directory> --platform <platform> --arch <arch> [--report <file>]',
    '  node scripts/shell-package-lifecycle.js coexistence --input <json> --platform <platform> --arch <arch> [--report <file>]',
  ].join('\n');
}

async function main(argv) {
  const [command, ...provided] = argv;
  if (provided[0] === '--') provided.shift();
  const args = [...provided];
  const platform = option(args, '--platform', process.platform);
  const architecture = option(args, '--arch', process.arch);
  const reportFile = option(args, '--report', undefined);
  let report;
  if (command === 'lifecycle') {
    const profileFile = option(args, '--profile', undefined);
    const consumerFile = option(args, '--consumer', undefined);
    const packageDirectory = option(args, '--package', undefined);
    if (!profileFile || !consumerFile || !packageDirectory || args.length > 0) fail(usage());
    report = await runPackageLifecycle({
      profileFile,
      consumerFile,
      packageDirectory,
      platform,
      architecture,
      target: `${platform}-${architecture}`,
    });
  } else if (command === 'coexistence') {
    const inputFile = option(args, '--input', undefined);
    if (!inputFile || args.length > 0) fail(usage());
    report = await runPackageCoexistence(readCoexistence(inputFile, platform, architecture));
  } else {
    fail(usage());
  }
  writeReport(reportFile, report);
  return report;
}

if (require.main === module) {
  main(process.argv.slice(2))
    .then((report) => process.stdout.write(`${JSON.stringify(report)}\n`))
    .catch((error) => {
      console.error(error instanceof Error ? error.message : 'shell lifecycle failed');
      process.exitCode = 1;
    });
}

module.exports = {
  assertEmptyRegistries,
  main,
  readCoexistence,
  runPackageCoexistence,
  runPackageLifecycle,
};
