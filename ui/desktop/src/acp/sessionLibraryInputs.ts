import { getAcpClient } from './acpConnection';
import type { ShellLibraryItemSummary } from '@repo-makeover/gosling-sdk';
import type { ImageData } from '../types/message';
import type { ResearchInitialInputs } from '../types/sessionExperience';
import {
  isResearchInitialImageFile,
  MAX_RESEARCH_INITIAL_FILE_BYTES,
  MAX_RESEARCH_INITIAL_IMAGE_BYTES,
  MAX_RESEARCH_INITIAL_INPUTS,
  MAX_RESEARCH_INITIAL_TEXT_BYTES,
  MAX_RESEARCH_INITIAL_TOTAL_IMAGE_BYTES,
  MAX_RESEARCH_INITIAL_TOTAL_TEXT_BYTES,
  researchInitialInputCount,
  researchInitialTextBytes,
} from '../types/sessionExperience';

export async function listSessionLibraryInputs(
  sessionId: string
): Promise<ShellLibraryItemSummary[]> {
  const client = await getAcpClient();
  const response = await client.gosling.shellSessionLibraryList_unstable({ sessionId });
  return response.items ?? [];
}

export async function addSessionLibraryText(
  sessionId: string,
  name: string,
  text: string
): Promise<ShellLibraryItemSummary> {
  if (!text.trim() || researchInitialTextBytes(text) > MAX_RESEARCH_INITIAL_TEXT_BYTES) {
    throw new Error('Pasted text must contain content and be no larger than 256 KB.');
  }
  const client = await getAcpClient();
  const response = await client.gosling.shellSessionLibraryAddText_unstable({
    sessionId,
    scope: 'session',
    name,
    text,
  });
  return response.item;
}

export async function linkSessionLibraryFile(
  sessionId: string,
  file: File
): Promise<ShellLibraryItemSummary> {
  if (
    file.size === 0 ||
    file.size > MAX_RESEARCH_INITIAL_FILE_BYTES ||
    (isResearchInitialImageFile(file.name) && file.size > MAX_RESEARCH_INITIAL_IMAGE_BYTES)
  ) {
    throw new Error('Choose a non-empty file up to 20 MB, or an image up to 5 MB.');
  }
  const path = window.electron.getPathForFile(file);
  const client = await getAcpClient();
  const response = await client.gosling.shellSessionLibraryLinkFile_unstable({
    sessionId,
    scope: 'session',
    path,
  });
  return response.item;
}

export async function addResearchInitialInputs(
  sessionId: string,
  inputs: ResearchInitialInputs
): Promise<string[]> {
  if (researchInitialInputCount(inputs) > MAX_RESEARCH_INITIAL_INPUTS) {
    throw new Error(
      `Research sessions support up to ${MAX_RESEARCH_INITIAL_INPUTS} initial inputs.`
    );
  }

  const texts = inputs.texts.map((text) => text.trim()).filter(Boolean);
  const textSizes = texts.map(researchInitialTextBytes);
  if (
    textSizes.some((size) => size > MAX_RESEARCH_INITIAL_TEXT_BYTES) ||
    textSizes.reduce((total, size) => total + size, 0) > MAX_RESEARCH_INITIAL_TOTAL_TEXT_BYTES
  ) {
    throw new Error('Initial research text exceeds the ACP input limits.');
  }

  const imageBytes = inputs.files
    .filter((file) => isResearchInitialImageFile(file.name))
    .map((file) => file.sizeBytes);
  if (
    inputs.files.some((file) => file.sizeBytes > MAX_RESEARCH_INITIAL_FILE_BYTES) ||
    imageBytes.some((size) => size > MAX_RESEARCH_INITIAL_IMAGE_BYTES) ||
    imageBytes.reduce((total, size) => total + size, 0) > MAX_RESEARCH_INITIAL_TOTAL_IMAGE_BYTES
  ) {
    throw new Error('Initial research files exceed the ACP input limits.');
  }

  const client = await getAcpClient();
  const itemIds: string[] = [];

  for (const [index, text] of texts.entries()) {
    const response = await client.gosling.shellSessionLibraryAddText_unstable({
      sessionId,
      scope: 'session',
      name: `Initial research input ${index + 1}`,
      text,
    });
    itemIds.push(response.item.id);
  }

  for (const file of inputs.files) {
    const response = await client.gosling.shellSessionLibraryLinkFile_unstable({
      sessionId,
      scope: 'session',
      path: file.path,
    });
    itemIds.push(response.item.id);
  }

  return itemIds;
}

export async function resolveSessionLibraryInputs(
  sessionId: string,
  itemIds: string[]
): Promise<{ assistantContext?: string; images: ImageData[] }> {
  if (itemIds.length === 0) return { images: [] };

  const client = await getAcpClient();
  const response = await client.gosling.shellSessionLibraryResolve_unstable({
    sessionId,
    itemIds,
  });
  const text: string[] = [];
  const images: ImageData[] = [];

  for (const item of response.items ?? []) {
    if (item.content.type === 'text') {
      text.push(item.content.text);
    } else {
      images.push({ data: item.content.data, mimeType: item.content.mime_type });
    }
  }

  return {
    ...(text.length > 0 ? { assistantContext: text.join('\n\n') } : {}),
    images,
  };
}
