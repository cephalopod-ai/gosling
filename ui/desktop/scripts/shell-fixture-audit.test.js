const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const repositoryRoot = path.resolve(__dirname, '..', '..', '..');
const fixtureRoot = path.join(repositoryRoot, 'fixtures', 'shell-products');
const forbiddenDomainTerms = [
  'dawes',
  'project abc',
  'math_mcp',
  'physics',
  'cst',
  'domainadapter',
  'prompt',
  'payload',
  'branding',
];
const forbiddenReleaseTerms = ['releaseDestination', 'updateUrl', 'feedUrl', 'publishable": true'];

function fixtureText(directory) {
  return fs
    .readdirSync(directory, { recursive: true, withFileTypes: true })
    .filter((entry) => entry.isFile() && ['.json', '.svg'].includes(path.extname(entry.name)))
    .map((entry) => fs.readFileSync(path.join(entry.parentPath, entry.name), 'utf8'))
    .join('\n')
    .toLowerCase();
}

test('neutral fixture sources contain no domain semantics or release activation', () => {
  for (const fixture of ['fixture-a', 'fixture-b']) {
    const text = fixtureText(path.join(fixtureRoot, fixture));
    for (const term of forbiddenDomainTerms) {
      assert.equal(text.includes(term.toLowerCase()), false, `${fixture} contains domain term ${term}`);
    }
    for (const term of forbiddenReleaseTerms) {
      assert.equal(text.includes(term.toLowerCase()), false, `${fixture} contains release term ${term}`);
    }
  }
});
