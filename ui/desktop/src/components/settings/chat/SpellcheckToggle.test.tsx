import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { SpellcheckToggle } from './SpellcheckToggle';

describe('SpellcheckToggle', () => {
  beforeEach(() => {
    Object.assign(window.electron, {
      getSpellcheckState: vi.fn(async () => true),
      setSpellcheck: vi.fn(async () => undefined),
    });
  });

  it('names the switch after the setting it controls', () => {
    render(<SpellcheckToggle />, { wrapper: IntlTestWrapper });

    expect(screen.getByRole('switch', { name: 'Enable Spellcheck' })).toBeInTheDocument();
  });
});
