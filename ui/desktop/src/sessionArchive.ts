import { acpArchiveSession, acpExportSession } from './acp/sessions';

const ARCHIVE_FILE_EXTENSION = '.json';

export class ArchiveFolderNotConfiguredError extends Error {
  constructor() {
    super('Archive folder is not configured');
    this.name = 'ArchiveFolderNotConfiguredError';
  }
}

function pathSeparator(dirPath: string): string {
  return dirPath.includes('\\') ? '\\' : '/';
}

function joinPath(dirPath: string, fileName: string): string {
  const separator = pathSeparator(dirPath);
  return `${dirPath.replace(/[\\/]+$/, '')}${separator}${fileName}`;
}

function sanitizeFileNamePart(value: string): string {
  const sanitized = value
    .trim()
    .replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '-')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^[.-]+|[.-]+$/g, '');

  return sanitized.length > 0 ? sanitized : 'session';
}

function archiveTimestampForFileName(timestamp: string): string {
  return timestamp.replace(/[:.]/g, '-');
}

async function getTrackedArchiveFiles(): Promise<Record<string, string>> {
  return (await window.electron.getSetting('archivedSessionFiles')) ?? {};
}

async function setTrackedArchiveFiles(files: Record<string, string>): Promise<void> {
  await window.electron.setSetting('archivedSessionFiles', files);
}

export async function getArchiveFolder(): Promise<string | null> {
  return await window.electron.getSetting('archiveFolder');
}

export async function getTrackedArchiveFile(sessionId: string): Promise<string | undefined> {
  const files = await getTrackedArchiveFiles();
  return files[sessionId];
}

export async function archiveSessionToConfiguredFolder(
  sessionId: string,
  sessionName: string
): Promise<{ archivedAt: string; filePath: string }> {
  const archiveFolder = await getArchiveFolder();
  if (!archiveFolder) {
    throw new ArchiveFolderNotConfiguredError();
  }

  const archivedAt = new Date().toISOString();
  const filePath = joinPath(
    archiveFolder,
    `${archiveTimestampForFileName(archivedAt)}-${sanitizeFileNamePart(sessionName)}-${sessionId}${ARCHIVE_FILE_EXTENSION}`
  );
  const previousFilePath = await getTrackedArchiveFile(sessionId);
  const exportedSession = await acpExportSession(sessionId);

  if (!(await window.electron.ensureDirectory(archiveFolder))) {
    throw new Error(`Failed to create archive directory: ${archiveFolder}`);
  }
  if (!(await window.electron.writeFile(filePath, exportedSession))) {
    throw new Error(`Failed to write archive file: ${filePath}`);
  }

  try {
    await acpArchiveSession(sessionId);
  } catch (error) {
    await window.electron.deleteFile(filePath);
    throw error;
  }

  const nextFiles = {
    ...(await getTrackedArchiveFiles()),
    [sessionId]: filePath,
  };
  await setTrackedArchiveFiles(nextFiles);

  if (previousFilePath && previousFilePath !== filePath) {
    await window.electron.deleteFile(previousFilePath);
  }

  return { archivedAt, filePath };
}

export async function removeTrackedArchiveFile(sessionId: string): Promise<{
  filePath?: string;
  hadTrackedFile: boolean;
  removed: boolean;
}> {
  const trackedFiles = await getTrackedArchiveFiles();
  const filePath = trackedFiles[sessionId];
  if (!filePath) {
    return { hadTrackedFile: false, removed: false };
  }

  const { [sessionId]: _removed, ...remainingFiles } = trackedFiles;
  await setTrackedArchiveFiles(remainingFiles);

  const removed = await window.electron.deleteFile(filePath);
  return { filePath, hadTrackedFile: true, removed };
}
