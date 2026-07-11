#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

const projectDir = path.join(__dirname, '..');
const messagesDir = path.join(projectDir, 'src', 'i18n', 'messages');
const sourcePath = path.join(messagesDir, 'en.json');
const source = JSON.parse(fs.readFileSync(sourcePath, 'utf8'));

for (const file of fs
  .readdirSync(messagesDir)
  .filter((file) => file.endsWith('.json') && file !== 'en.json')) {
  const localePath = path.join(messagesDir, file);
  const locale = JSON.parse(fs.readFileSync(localePath, 'utf8'));
  const synchronized = Object.fromEntries(
    Object.entries(locale).filter(([key]) => Object.hasOwn(source, key))
  );

  for (const [key, message] of Object.entries(source)) {
    if (!Object.hasOwn(synchronized, key)) synchronized[key] = message;
  }

  const output = `${JSON.stringify(synchronized, null, 2)}\n`;

  if (fs.readFileSync(localePath, 'utf8') !== output) {
    fs.writeFileSync(localePath, output);
  }
}
