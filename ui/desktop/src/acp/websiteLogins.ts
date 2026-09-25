import type { WebsiteLoginDto } from '@repo-makeover/gosling-sdk';
import { getAcpClient } from './acpConnection';

export type { WebsiteLoginDto };

export interface WebsiteLoginDraft {
  id?: string;
  name: string;
  url: string;
  username: string;
  /** Omit when editing to keep the saved password. */
  password?: string;
}

export async function listWebsiteLogins(): Promise<WebsiteLoginDto[]> {
  const client = await getAcpClient();
  const { logins } = await client.gosling.websiteLoginsList_unstable({});
  return logins;
}

export async function saveWebsiteLogin(draft: WebsiteLoginDraft): Promise<WebsiteLoginDto> {
  const client = await getAcpClient();
  const { login } = await client.gosling.websiteLoginsSave_unstable({
    id: draft.id ?? null,
    name: draft.name,
    url: draft.url,
    username: draft.username,
    password: draft.password ? draft.password : null,
  });
  return login;
}

export async function deleteWebsiteLogin(id: string): Promise<void> {
  const client = await getAcpClient();
  await client.gosling.websiteLoginsDelete_unstable({ id });
}
