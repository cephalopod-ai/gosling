import { describe, expect, it } from 'vitest';
import { NAV_ITEMS } from './useNavigationItems';

describe('NAV_ITEMS', () => {
  it('places New Research directly below New Chat', () => {
    expect(NAV_ITEMS.slice(0, 2).map(({ id, label, path }) => ({ id, label, path }))).toEqual([
      { id: 'home', label: 'New Chat', path: '/' },
      { id: 'research', label: 'New Research', path: '/research' },
    ]);
  });

  it('keeps catalog management out of the primary navigation', () => {
    expect(NAV_ITEMS.map(({ id }) => id)).toEqual(['home', 'research', 'sessions']);
  });
});
