function applyTheme(isDark: boolean): void {
  document.documentElement.classList.toggle('dark', isDark);
  document.documentElement.style.colorScheme = isDark ? 'dark' : 'light';
}

function initializeTheme(): void {
  try {
    const systemPrefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const useSystemTheme = window.localStorage?.getItem('use_system_theme') === 'true';
    const savedTheme = window.localStorage?.getItem('theme');
    const isDark = useSystemTheme
      ? systemPrefersDark
      : savedTheme
        ? savedTheme === 'dark'
        : systemPrefersDark;

    applyTheme(isDark);
  } catch (error) {
    console.warn('Failed to initialize theme from localStorage, using system preference:', error);
    applyTheme(window.matchMedia('(prefers-color-scheme: dark)').matches);
  }
}

initializeTheme();

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', () => {
    setTimeout(initializeTheme, 50);
  });
}
