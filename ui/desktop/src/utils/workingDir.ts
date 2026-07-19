export const getInitialWorkingDir = (): string => {
  // Fall back to initial config from app startup
  return (window.appConfig?.get('GOSLING_WORKING_DIR') as string) ?? '';
};

export const getDefaultWorkspaceWorkingDir = (): string => {
  const home = (window.appConfig?.get('GOSLING_HOME_DIR') as string) || getInitialWorkingDir();
  const separator = home.includes('\\') ? '\\' : '/';
  return `${home.replace(/[\\/]$/, '')}${separator}Work`;
};
