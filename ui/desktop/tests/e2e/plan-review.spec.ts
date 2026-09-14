import { expect, test } from './fixtures';

const layouts = [
  { name: 'standard', width: 1280, height: 800 },
  { name: 'compact', width: 390, height: 700 },
] as const;

test.setTimeout(120_000);

test('plan review remains usable in compact and standard windows', async ({
  goslingPage,
}, testInfo) => {
  for (const layout of layouts) {
    await goslingPage.setViewportSize({ width: layout.width, height: layout.height });
    await goslingPage.evaluate(() => {
      document.querySelector('[data-testid="plan-review-playtest-frame"]')?.remove();
      const frame = document.createElement('iframe');
      frame.dataset.testid = 'plan-review-playtest-frame';
      frame.src = new URL('/plan-review-playtest.html', window.location.origin).toString();
      Object.assign(frame.style, {
        position: 'fixed',
        inset: '0',
        width: '100vw',
        height: '100vh',
        border: '0',
        zIndex: '2147483647',
      });
      document.body.append(frame);
    });
    const frameElement = await goslingPage.waitForSelector(
      '[data-testid="plan-review-playtest-frame"]'
    );
    const playtestPage = await frameElement.contentFrame();
    if (!playtestPage) throw new Error('Plan review playtest frame did not load');

    const dialog = playtestPage.getByRole('dialog', { name: 'Plan review' });
    await expect(dialog).toBeVisible();
    await expect(dialog).toHaveAccessibleDescription(
      /Revision 7 · 0123456789 · source through 87 · gpt-5/
    );

    const box = await dialog.boundingBox();
    expect(box).not.toBeNull();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.y).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(layout.width);
    expect(box!.y + box!.height).toBeLessThanOrEqual(layout.height);

    await goslingPage.screenshot({
      path: testInfo.outputPath(`plan-review-${layout.name}.png`),
      fullPage: false,
    });

    const feedback = playtestPage.getByLabel('Feedback');
    await feedback.scrollIntoViewIfNeeded();
    await expect(feedback).toHaveValue('Retain this draft while the review window is open.');
    await expect(playtestPage.getByRole('button', { name: 'Request changes' })).toBeVisible();
    await expect(playtestPage.getByRole('button', { name: 'Approve', exact: true })).toBeVisible();
    await expect(playtestPage.getByRole('button', { name: 'Approve and implement' })).toBeVisible();

    await feedback.focus();
    await feedback.press('End');
    await feedback.type(` ${layout.name}`);
    await playtestPage.getByRole('button', { name: 'Approve', exact: true }).click();
    await expect
      .poll(() => playtestPage.evaluate(() => window.planReviewPlaytest.lastAction))
      .toBe('approve');
  }
});
