import { expect, test } from './fixtures';

test.setTimeout(240_000);

test('Deep Research Initial Inputs wraps long source material without widening the dialog', async ({
  goslingPage,
}) => {
  const reload = goslingPage.getByRole('button', { name: 'Reload' });
  if (await reload.isVisible().catch(() => false)) {
    await reload.click();
    await goslingPage.waitForLoadState('domcontentloaded');
  }
  await goslingPage.evaluate(() => {
    window.location.hash = '/research';
  });
  await expect(goslingPage.getByRole('heading', { name: 'Deep Research' })).toBeVisible();

  await goslingPage.getByRole('button', { name: /Initial Inputs/ }).click();
  const dialog = goslingPage.getByRole('dialog');
  await expect(dialog).toBeVisible();
  const textarea = goslingPage.getByLabel('Paste content');
  const longInput = `# Evidence\n\n${'https://example.test/'.concat('unbroken'.repeat(300))}`;
  await textarea.fill(longInput);
  await goslingPage.getByRole('button', { name: 'Next' }).click();

  const layout = await dialog.evaluate((element) => ({
    clientWidth: element.clientWidth,
    scrollWidth: element.scrollWidth,
    bottom: element.getBoundingClientRect().bottom,
    viewportHeight: window.innerHeight,
  }));
  expect(layout.scrollWidth).toBe(layout.clientWidth);
  expect(layout.bottom).toBeLessThanOrEqual(layout.viewportHeight);
  await expect(goslingPage.getByText(longInput, { exact: true })).toBeVisible();
  await goslingPage.screenshot({
    path: test.info().outputPath('deep-research-initial-input-wrap.png'),
    fullPage: true,
  });
});
