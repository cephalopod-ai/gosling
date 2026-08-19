/**
 * The shell settings schema and its bounds, with no Node or Electron dependency, so the renderer can
 * render the same constraints the main-process store enforces without pulling `node:fs` into the
 * browser bundle. `localSettings.ts` re-exports every symbol here, so existing importers are
 * unaffected and there is still exactly one source of truth.
 */

export const SHELL_SETTINGS_SCHEMA_VERSION = 1;

export type ShellTheme = 'system' | 'light' | 'dark';

export const SHELL_THEME_VALUES: readonly ShellTheme[] = ['system', 'light', 'dark'];

export const MIN_SHELL_TEXT_SCALE = 0.8;
export const MAX_SHELL_TEXT_SCALE = 2;

export function isValidShellTheme(value: unknown): value is ShellTheme {
  return typeof value === 'string' && (SHELL_THEME_VALUES as readonly string[]).includes(value);
}

export function isValidShellTextScale(value: unknown): value is number {
  return (
    typeof value === 'number' &&
    Number.isFinite(value) &&
    value >= MIN_SHELL_TEXT_SCALE &&
    value <= MAX_SHELL_TEXT_SCALE
  );
}
