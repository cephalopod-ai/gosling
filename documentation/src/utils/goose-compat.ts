import type { MCPServer } from "../types/server";
import type { Skill } from "@site/src/pages/skills/types";

export const GOOSE_SERVERS_URL = "https://goose-docs.ai/servers.json";
export const GOOSE_SKILLS_MANIFEST_URL =
  "https://goose-docs.ai/skills-manifest.json";
export const GOOSE_COMPATIBILITY_NOTE =
  "Imported from Goose's AAIF-maintained catalog and normalized for gosling compatibility.";
export const GOOSE_EXCLUDED_SKILL_IDS = new Set([
  "code-review",
  "testing-strategy",
]);

function asString(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function asBoolean(value: unknown, fallback = false): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function convertGooseText(value: unknown): string {
  return asString(value)
    .replace(/goose:\/\//gi, "gosling://")
    .replace(/\bgoose session\b/gi, "gosling session")
    .replace(/\bgoose mcp\b/gi, "gosling mcp")
    .replace(/\bgoose\b/gi, "gosling");
}

function convertGooseCommand(value: unknown): string {
  return asString(value)
    .replace(/goose:\/\//gi, "gosling://")
    .replace(/\bgoose\b/gi, "gosling");
}

function normalizeRequiredEntry(entry: any): {
  name: string;
  description: string;
  required: boolean;
} {
  return {
    name: convertGooseText(entry?.name),
    description: convertGooseText(
      entry?.description || "Required environment variable",
    ),
    required: asBoolean(entry?.required),
  };
}

function normalizeServerType(type: unknown): MCPServer["type"] | undefined {
  if (type === "streamable_http") return "streamable-http";
  if (type === "local" || type === "remote" || type === "streamable-http") {
    return type;
  }
  return undefined;
}

export function normalizeGooseServer(server: any): MCPServer {
  const id = asString(server?.id);
  const normalized: MCPServer = {
    id,
    name: convertGooseText(server?.name || id),
    description: convertGooseText(
      server?.description || "No description provided.",
    ),
    link: asString(server?.link),
    installation_notes: convertGooseText(server?.installation_notes),
    is_builtin: asBoolean(server?.is_builtin),
    endorsed: asBoolean(server?.endorsed),
    environmentVariables: asArray(server?.environmentVariables).map(
      normalizeRequiredEntry,
    ),
    sourceCatalog: "goose",
    sourceCatalogUrl: GOOSE_SERVERS_URL,
    compatibilityNote: GOOSE_COMPATIBILITY_NOTE,
  };

  const command = convertGooseCommand(server?.command);
  if (command) normalized.command = command;

  const url = asString(server?.url);
  if (url) normalized.url = url;

  const type = normalizeServerType(server?.type);
  if (type) normalized.type = type;

  const headers = asArray(server?.headers).map(normalizeRequiredEntry);
  if (headers.length > 0) normalized.headers = headers;

  if (server?.show_install_link === false) normalized.show_install_link = false;
  if (server?.show_install_command === false) {
    normalized.show_install_command = false;
  }

  return normalized;
}

export function normalizeGoslingServer(server: any): MCPServer {
  return {
    id: asString(server?.id),
    name: asString(server?.name || server?.id),
    description: asString(server?.description || "No description provided."),
    command: asString(server?.command) || undefined,
    url: asString(server?.url) || undefined,
    type: normalizeServerType(server?.type),
    link: asString(server?.link),
    installation_notes: asString(server?.installation_notes),
    is_builtin: asBoolean(server?.is_builtin),
    endorsed: asBoolean(server?.endorsed),
    show_install_link: server?.show_install_link,
    show_install_command: server?.show_install_command,
    environmentVariables: asArray(server?.environmentVariables).map(
      normalizeRequiredEntry,
    ),
    headers: asArray(server?.headers).map(normalizeRequiredEntry),
  };
}

export function normalizeGooseSkill(skill: any): Skill {
  const id = asString(skill?.id);
  const sourceUrl = asString(skill?.sourceUrl || skill?.source_url);
  const repoUrl = asString(skill?.repoUrl || skill?.repo_url || sourceUrl);
  const author = asString(skill?.author || "goose");

  return {
    id,
    name: convertGooseText(skill?.name || id),
    description: convertGooseText(
      skill?.description || "No description provided.",
    ),
    author: author === "goose" ? "gosling via goose" : convertGooseText(author),
    version: asString(skill?.version) || undefined,
    status: skill?.status === "experimental" ? "experimental" : "stable",
    tags: asArray(skill?.tags).map((tag) => asString(tag)).filter(Boolean),
    sourceUrl,
    content: asString(skill?.content),
    hasSupporting: asBoolean(skill?.hasSupporting),
    supportingFiles: asArray(skill?.supportingFiles)
      .map((file) => asString(file))
      .filter(Boolean),
    supportingFilesType: asString(skill?.supportingFilesType || "none") as any,
    installMethod: asString(skill?.installMethod || "npx-multi") as any,
    installCommand: convertGooseCommand(skill?.installCommand) || undefined,
    viewSourceUrl: asString(skill?.viewSourceUrl || sourceUrl || repoUrl),
    repoUrl,
    isCommunity: false,
    sourceCatalog: "goose",
    sourceCatalogUrl: GOOSE_SKILLS_MANIFEST_URL,
    compatibilityNote: GOOSE_COMPATIBILITY_NOTE,
  };
}

export function isSupportedGooseSkill(skill: any): boolean {
  return !GOOSE_EXCLUDED_SKILL_IDS.has(asString(skill?.id));
}

export function normalizeGoslingSkill(skill: any): Skill {
  return {
    ...skill,
    status: skill?.status === "experimental" ? "experimental" : "stable",
    tags: asArray(skill?.tags).map((tag) => asString(tag)).filter(Boolean),
    hasSupporting: asBoolean(skill?.hasSupporting),
    supportingFiles: asArray(skill?.supportingFiles)
      .map((file) => asString(file))
      .filter(Boolean),
    supportingFilesType: asString(skill?.supportingFilesType || "none") as any,
    installMethod: asString(skill?.installMethod || "download") as any,
    isCommunity: asBoolean(skill?.isCommunity),
  };
}

function compareByIdThenName<T extends { id: string; name: string }>(
  a: T,
  b: T,
): number {
  return `${a.id}\0${a.name}`.localeCompare(`${b.id}\0${b.name}`);
}

export function dedupeAndSortById<T extends { id: string; name: string }>(
  items: T[],
): T[] {
  const byId = new Map<string, T>();
  for (const item of items) {
    if (!item.id) {
      console.warn("goose-compat: dropping catalog entry with missing id", item);
      continue;
    }
    if (byId.has(item.id)) continue;
    byId.set(item.id, item);
  }
  return Array.from(byId.values()).sort(compareByIdThenName);
}
