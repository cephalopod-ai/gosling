import type { MCPServer } from "../types/server";
import {
  GOOSE_SERVERS_URL,
  dedupeAndSortById,
  normalizeGooseServer,
  normalizeGoslingServer,
} from "./goose-compat";

const SERVERS_URL = "/servers.json";

async function fetchCatalog(url: string): Promise<unknown | null> {
  const response = await fetch(url);
  if (!response.ok) return null;
  return response.json();
}

export async function fetchMCPServers(): Promise<MCPServer[]> {
  const catalogs = [
    { url: SERVERS_URL, normalize: normalizeGoslingServer },
    { url: GOOSE_SERVERS_URL, normalize: normalizeGooseServer },
  ];

  for (const catalog of catalogs) {
    try {
      const data = await fetchCatalog(catalog.url);
      if (!Array.isArray(data) || data.length === 0) continue;

      const normalized = dedupeAndSortById(data.map(catalog.normalize));
      if (normalized.length === 0) continue;

      return normalized;
    } catch (error) {
      console.error("Error fetching MCP servers:", catalog.url, error);
    }
  }

  return [];
}

export async function searchMCPServers(query: string): Promise<MCPServer[]> {
  const servers = await fetchMCPServers();
  const normalizedQuery = query.toLowerCase();

  return servers.filter((server) => {
    const normalizedName = server.name.toLowerCase();
    const normalizedDescription = server.description.toLowerCase();

    return (
      normalizedName.includes(normalizedQuery) ||
      normalizedDescription.includes(normalizedQuery)
    );
  });
}
