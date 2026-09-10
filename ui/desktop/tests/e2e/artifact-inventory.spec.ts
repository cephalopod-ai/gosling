import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from './fixtures';

test.setTimeout(240_000);

test('session inventory tabs show boxed counts before preview selection', async ({
  goslingPage,
}) => {
  const sessionId = 'artifact-inventory-playwright';
  await goslingPage.evaluate((id) => {
    window.location.hash = `/pair?resumeSessionId=${id}`;
  }, sessionId);
  await goslingPage.waitForTimeout(2_000);

  await goslingPage.evaluate(async (id) => {
    // The Electron test fixture runs against Vite's renderer server, so this imports the same
    // session-store module instance used by the mounted chat rather than a test replacement.
    // @ts-expect-error This URL exists in the browser-side Vite module graph, not Node resolution.
    const { acpChatSessionActions } = await import('/src/acp/chatSessionStore.ts');
    acpChatSessionActions.setArtifacts(
      id,
      ['report.md', 'brief.pdf', 'notes.txt', 'data.json'].map((displayPath, index) => ({
        sessionId: id,
        displayPath,
        resolvedPath: `/outputs/${displayPath}`,
        baseWorkingDir: '/outputs',
        relation: 'created',
        provenance: 'built_in_tool',
        sourceId: `tool-${index}`,
        firstSeenAt: `2026-01-01T00:00:0${index}Z`,
        lastSeenAt: `2026-01-01T00:00:0${index}Z`,
      }))
    );
  }, sessionId);

  await goslingPage.getByRole('button', { name: 'Toggle outputs pane' }).click();
  const outputsTab = goslingPage.getByRole('tab', { name: 'Outputs 4' });
  await expect(outputsTab).toBeVisible();
  await expect(goslingPage.getByTestId('outputs-count')).toHaveClass(/rounded-md/);
  await expect(goslingPage.getByTestId('outputs-count')).toHaveClass(/border/);
  await expect(goslingPage.getByText('report.md')).toBeVisible();
  await expect(goslingPage.getByText('brief.pdf')).toBeVisible();
  await expect(goslingPage.getByText('notes.txt')).toBeVisible();
  await expect(goslingPage.getByText('data.json')).toBeVisible();

  const inputsTab = goslingPage.getByRole('tab', { name: 'Inputs 0' });
  await expect(inputsTab).toBeVisible();
  await expect(goslingPage.getByTestId('inputs-count')).toHaveClass(/rounded-md/);
  const libraryTab = goslingPage.getByRole('tab', { name: /Library \d+/ });
  await expect(libraryTab).toBeVisible();
  await expect(goslingPage.getByTestId('library-count')).toHaveClass(/rounded-md/);
  await expect(goslingPage.getByTestId('library-count')).toHaveClass(/border/);
  await inputsTab.click();
  await expect(inputsTab).toHaveAttribute('aria-selected', 'true');
  await goslingPage.screenshot({
    path: test.info().outputPath('session-inventory-tabs.png'),
    fullPage: true,
  });
});

test('a qualified output replaces its dead bare alias and previews without a picker', async ({
  goslingPage,
}) => {
  const root = await mkdtemp(join(tmpdir(), 'gosling-output-alias-'));
  const outputs = join(root, 'Outputs');
  const outputPath = join(outputs, 'report.md');
  await mkdir(outputs);
  await writeFile(outputPath, '# Direct preview\n\nOpened from Outputs.');

  try {
    const sessionId = 'artifact-alias-playwright';
    await goslingPage.evaluate((id) => {
      window.location.hash = `/pair?resumeSessionId=${id}`;
    }, sessionId);
    await goslingPage.waitForTimeout(2_000);

    await goslingPage.evaluate(
      async ({ id, base, missing, resolved }) => {
        // @ts-expect-error This URL exists in the browser-side Vite module graph, not Node resolution.
        const { acpChatSessionActions } = await import('/src/acp/chatSessionStore.ts');
        const shared = {
          sessionId: id,
          baseWorkingDir: base,
          relation: 'referenced',
          provenance: 'assistant_message',
          sourceId: 'message-1',
          firstSeenAt: '2026-09-09T00:00:00Z',
          lastSeenAt: '2026-09-09T00:00:00Z',
        };
        acpChatSessionActions.setArtifacts(id, [
          {
            ...shared,
            displayPath: 'Outputs/report.md',
            resolvedPath: resolved,
          },
          {
            ...shared,
            displayPath: 'report.md',
            resolvedPath: missing,
          },
        ]);
      },
      { id: sessionId, base: root, missing: join(root, 'report.md'), resolved: outputPath }
    );

    await goslingPage.getByRole('button', { name: 'Toggle outputs pane' }).click();
    await expect(goslingPage.getByRole('tab', { name: 'Outputs 1' })).toBeVisible();
    await goslingPage.getByTitle(outputPath).click();
    await expect(goslingPage.getByRole('heading', { name: 'Direct preview' })).toBeVisible();
    await expect(
      goslingPage.getByText('Select this file to grant access and preview it')
    ).toHaveCount(0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
