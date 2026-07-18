interface WorkspaceWorkingFolder {
  id: string;
  workingFolder: string;
}

export function reconcileWorkspaceWorkingDir(
  current: string,
  previous: WorkspaceWorkingFolder | undefined,
  active: WorkspaceWorkingFolder
): string {
  if (!previous || previous.id !== active.id || current === previous.workingFolder) {
    return active.workingFolder;
  }
  return current;
}
