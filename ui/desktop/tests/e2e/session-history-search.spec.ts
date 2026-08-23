import { expect, test } from './fixtures';

test('shows and focuses the session history keyword search', async ({ goslingPage }) => {
  await goslingPage.getByRole('button', { name: 'Session History' }).click();

  const search = goslingPage.getByRole('searchbox', { name: 'Search chat history' });
  await expect(search).toBeVisible();
  await search.fill('keyword');
  await expect(search).toHaveValue('keyword');

  await goslingPage.evaluate(() => {
    window.dispatchEvent(
      new KeyboardEvent('keydown', { bubbles: true, cancelable: true, key: 'f', metaKey: true })
    );
  });
  await expect(search).toBeFocused();
});
