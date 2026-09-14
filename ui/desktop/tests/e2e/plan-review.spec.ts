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
    await goslingPage.goto('http://localhost:5173/plan-review-playtest.html', {
      // Electron's development connection can remain busy after Vite commits the
      // dedicated playtest page. The dialog assertion below is the readiness gate.
      waitUntil: 'commit',
    });

    const dialog = goslingPage.getByRole('dialog', { name: 'Plan review' });
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

    const feedback = goslingPage.getByLabel('Feedback');
    await feedback.scrollIntoViewIfNeeded();
    await expect(feedback).toHaveValue('Retain this draft while the review window is open.');
    await expect(goslingPage.getByRole('button', { name: 'Request changes' })).toBeVisible();
    await expect(goslingPage.getByRole('button', { name: 'Approve', exact: true })).toBeVisible();
    await expect(goslingPage.getByRole('button', { name: 'Approve and implement' })).toBeVisible();

    await feedback.focus();
    await feedback.press('End');
    await feedback.type(` ${layout.name}`);
    await goslingPage.getByRole('button', { name: 'Approve', exact: true }).click();
    await expect
      .poll(() => goslingPage.evaluate(() => window.planReviewPlaytest.lastAction))
      .toBe('approve');
  }
});
