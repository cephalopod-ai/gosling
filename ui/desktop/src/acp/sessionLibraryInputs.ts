import { getAcpClient } from './acpConnection';
import type { ImageData } from '../types/message';
import type { ResearchInitialInputs } from '../types/sessionExperience';
import { MAX_RESEARCH_INITIAL_INPUTS, researchInitialInputCount } from '../types/sessionExperience';

export async function addResearchInitialInputs(
  sessionId: string,
  inputs: ResearchInitialInputs
): Promise<string[]> {
  if (researchInitialInputCount(inputs) > MAX_RESEARCH_INITIAL_INPUTS) {
    throw new Error(
      `Research sessions support up to ${MAX_RESEARCH_INITIAL_INPUTS} initial inputs.`
    );
  }

  const client = await getAcpClient();
  const itemIds: string[] = [];
  const text = inputs.text.trim();

  if (text) {
    const response = await client.gosling.shellSessionLibraryAddText_unstable({
      sessionId,
      scope: 'session',
      name: 'Initial research notes',
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
