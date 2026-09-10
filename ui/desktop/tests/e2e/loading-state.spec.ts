import { CHAT_SUBMIT_SHORTCUT, test, expect } from './fixtures';

test.describe('Loading State', () => {
  test('uses Enter for line breaks and the platform shortcut to send', async ({ goslingPage }) => {
    const chatInput = await goslingPage.waitForSelector('[data-testid="chat-input"]', {
      timeout: 30000,
    });

    await chatInput.fill('First line');
    await chatInput.press('Enter');
    await chatInput.type('Second line');

    expect(await chatInput.inputValue()).toBe('First line\nSecond line');

    await chatInput.press(CHAT_SUBMIT_SHORTCUT);
    await goslingPage.waitForSelector('[data-testid="loading-indicator"]', {
      state: 'visible',
      timeout: 10000,
    });
  });

  test('shows a model placeholder while creating a new chat session', async ({ goslingPage }) => {
    await goslingPage.waitForSelector('[data-testid="chat-input"]', { timeout: 30000 });

    const chatInput = await goslingPage.waitForSelector('[data-testid="chat-input"]');
    await chatInput.fill('Respond with the single word hello.');
    await chatInput.press(CHAT_SUBMIT_SHORTCUT);

    await goslingPage.waitForSelector('[data-testid="loading-indicator"]', {
      state: 'visible',
      timeout: 10000,
    });

    const loadingModel = goslingPage.locator('[data-testid="model-loading-state"]');
    await expect(loadingModel).toHaveText(/loading model/i, { timeout: 10000 });

    await goslingPage.screenshot({
      path: test.info().outputPath('loading-state-fresh-session.png'),
      fullPage: true,
    });
  });
});
