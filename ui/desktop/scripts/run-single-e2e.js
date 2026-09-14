const { spawn } = require('node:child_process');

const forwarded = process.argv.slice(2);
while (forwarded[0] === '--') {
  forwarded.shift();
}

if (forwarded.length === 0) {
  console.error('Usage: pnpm run test-e2e:single -- <title pattern | Playwright arguments>');
  process.exit(2);
}

const playwrightArguments = forwarded[0].startsWith('-') ? forwarded : ['--grep', ...forwarded];
const playwrightCli = require.resolve('@playwright/test/cli');
const child = spawn(process.execPath, [playwrightCli, 'test', ...playwrightArguments], {
  stdio: 'inherit',
});

child.on('error', (error) => {
  console.error(error);
  process.exit(1);
});

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
