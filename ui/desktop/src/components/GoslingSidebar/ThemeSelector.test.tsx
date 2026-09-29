import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useTheme } from '../../contexts/ThemeContext';
import { IntlTestWrapper } from '../../i18n/test-utils';
import ThemeSelector from './ThemeSelector';

vi.mock('../../contexts/ThemeContext', () => ({
  useTheme: vi.fn(),
}));

describe('ThemeSelector', () => {
  it('exposes the selected theme as the pressed button', () => {
    vi.mocked(useTheme).mockReturnValue({
      userThemePreference: 'dark',
      setUserThemePreference: vi.fn(),
    } as unknown as ReturnType<typeof useTheme>);

    render(<ThemeSelector hideTitle horizontal />, { wrapper: IntlTestWrapper });

    expect(screen.getByRole('button', { name: 'Dark', pressed: true })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Light', pressed: false })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'System', pressed: false })).toBeInTheDocument();
  });
});
