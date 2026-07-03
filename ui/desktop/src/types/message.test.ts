import { describe, expect, it } from 'vitest';
import { createUserMessage, getTextAndImageContent } from './message';

describe('message helpers', () => {
  it('keeps assistant-only context out of rendered user text', () => {
    const message = createUserMessage('visible prompt', [], 'hidden credential context');

    expect(getTextAndImageContent(message).textContent).toBe('visible prompt');
  });
});
