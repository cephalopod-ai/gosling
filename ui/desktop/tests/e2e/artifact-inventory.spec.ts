import { expect, test } from './fixtures';

test.setTimeout(240_000);

test('four discovered files populate Outputs before preview selection', async ({ goslingPage }) => {
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
      ['report.md', 'analysis.py', 'engine.rs', 'build.sh'].map((displayPath, index) => ({
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
  await expect(goslingPage.getByText('Outputs 4')).toBeVisible();
  await expect(goslingPage.getByText('report.md')).toBeVisible();
  await expect(goslingPage.getByText('analysis.py')).toBeVisible();
  await expect(goslingPage.getByText('engine.rs')).toBeVisible();
  await expect(goslingPage.getByText('build.sh')).toBeVisible();
});
