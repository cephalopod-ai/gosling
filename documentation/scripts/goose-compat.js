const fs = require("fs");
const assert = require("node:assert");

const GOOSE_SERVERS_URL = "https://goose-docs.ai/servers.json";
const GOOSE_SKILLS_MANIFEST_URL = "https://goose-docs.ai/skills-manifest.json";
const GOOSE_COMPATIBILITY_NOTE =
  "Imported from Goose's AAIF-maintained catalog and normalized for gosling compatibility.";

function asString(value, fallback = "") {
  return typeof value === "string" ? value : fallback;
}

function asBoolean(value, fallback = false) {
  return typeof value === "boolean" ? value : fallback;
}

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function convertGooseText(value) {
  return asString(value)
    .replace(/goose:\/\//gi, "gosling://")
    .replace(/\bgoose session\b/gi, "gosling session")
    .replace(/\bgoose mcp\b/gi, "gosling mcp")
    .replace(/\bgoose\b/gi, "gosling");
}

function convertGooseCommand(value) {
  return asString(value)
    .replace(/goose:\/\//gi, "gosling://")
    .replace(/\bgoose\b/gi, "gosling");
}

function normalizeNameDescriptionRequired(entry) {
  return {
    name: convertGooseText(entry?.name),
    description: convertGooseText(entry?.description || "Required environment variable"),
    required: asBoolean(entry?.required),
  };
}

function normalizeServerType(type) {
  if (type === "streamable_http") return "streamable-http";
  if (type === "local" || type === "remote" || type === "streamable-http") return type;
  return undefined;
}

function normalizeMcpServer(server) {
  const id = asString(server?.id);
  const normalized = {
    id,
    name: convertGooseText(server?.name || id),
    description: convertGooseText(server?.description || "No description provided."),
    link: asString(server?.link),
    installation_notes: convertGooseText(server?.installation_notes),
    is_builtin: asBoolean(server?.is_builtin),
    endorsed: asBoolean(server?.endorsed),
    environmentVariables: asArray(server?.environmentVariables).map(
      normalizeNameDescriptionRequired,
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

  const headers = asArray(server?.headers).map(normalizeNameDescriptionRequired);
  if (headers.length > 0) normalized.headers = headers;

  if (server?.show_install_link === false) normalized.show_install_link = false;
  if (server?.show_install_command === false) normalized.show_install_command = false;

  return normalized;
}

function normalizeSkill(skill) {
  const id = asString(skill?.id);
  const sourceUrl = asString(skill?.sourceUrl || skill?.source_url);
  const repoUrl = asString(skill?.repoUrl || skill?.repo_url || sourceUrl);
  const author = asString(skill?.author || "goose");

  const normalized = {
    id,
    name: convertGooseText(skill?.name || id),
    description: convertGooseText(skill?.description || "No description provided."),
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
    supportingFilesType: asString(skill?.supportingFilesType || "none"),
    installMethod: asString(skill?.installMethod || "npx-multi"),
    installCommand: convertGooseCommand(skill?.installCommand),
    viewSourceUrl: asString(skill?.viewSourceUrl || sourceUrl || repoUrl),
    repoUrl,
    isCommunity: false,
    sourceCatalog: "goose",
    sourceCatalogUrl: GOOSE_SKILLS_MANIFEST_URL,
    compatibilityNote: GOOSE_COMPATIBILITY_NOTE,
  };

  return Object.fromEntries(
    Object.entries(normalized).filter(([, value]) => value !== undefined),
  );
}

function compareByIdThenName(a, b) {
  return `${a.id || ""}\0${a.name || ""}`.localeCompare(`${b.id || ""}\0${b.name || ""}`);
}

function dedupeById(items) {
  const byId = new Map();
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

function normalizeServersCatalog(raw) {
  return dedupeById(asArray(raw).map(normalizeMcpServer));
}

function normalizeSkillsManifest(raw) {
  const skills = dedupeById(asArray(raw?.skills).map(normalizeSkill));
  return {
    skills,
    count: skills.length,
    sourceRepo: asString(raw?.sourceRepo || "https://github.com/block/Agent-Skills"),
    sourceCatalog: "goose",
    sourceCatalogUrl: GOOSE_SKILLS_MANIFEST_URL,
    compatibilityNote: GOOSE_COMPATIBILITY_NOTE,
  };
}

function stableSort(value) {
  if (Array.isArray(value)) return value.map(stableSort);
  if (!value || typeof value !== "object") return value;

  return Object.keys(value)
    .sort()
    .reduce((acc, key) => {
      acc[key] = stableSort(value[key]);
      return acc;
    }, {});
}

function stableStringify(value) {
  return `${JSON.stringify(stableSort(value), null, 2)}\n`;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function writeJson(filePath, value) {
  fs.writeFileSync(filePath, stableStringify(value));
}

function runSelfTest() {
  const server = normalizeMcpServer({
    id: "demo",
    name: "goose Demo",
    command: "goose mcp demo",
    installation_notes: "This comes with goose.",
    environmentVariables: [{ name: "TOKEN", required: true }],
    type: "streamable_http",
  });
  assert.strictEqual(server.name, "gosling Demo");
  assert.strictEqual(server.command, "gosling mcp demo");
  assert.strictEqual(server.installation_notes, "This comes with gosling.");
  assert.strictEqual(server.type, "streamable-http");
  assert.strictEqual(server.environmentVariables[0].description, "Required environment variable");

  const manifest = normalizeSkillsManifest({
    skills: [
      {
        id: "demo",
        name: "goose skill",
        author: "goose",
        repoUrl: "https://github.com/block/Agent-Skills",
        installCommand: "npx skills add https://github.com/block/Agent-Skills --skill demo",
      },
    ],
  });
  assert.strictEqual(manifest.skills.length, 1);
  assert.strictEqual(manifest.skills[0].name, "gosling skill");
  assert.strictEqual(manifest.skills[0].author, "gosling via goose");
  assert.strictEqual(manifest.count, 1);
}

function main(argv) {
  const [mode, inputPath, outputPath] = argv;

  if (mode === "--self-test") {
    runSelfTest();
    return;
  }

  if (!["--servers", "--skills"].includes(mode) || !inputPath || !outputPath) {
    console.error(
      "Usage: node scripts/goose-compat.js --servers <input.json> <output.json>\n" +
        "       node scripts/goose-compat.js --skills <input.json> <output.json>\n" +
        "       node scripts/goose-compat.js --self-test",
    );
    process.exit(1);
  }

  const raw = readJson(inputPath);
  const normalized =
    mode === "--servers" ? normalizeServersCatalog(raw) : normalizeSkillsManifest(raw);
  writeJson(outputPath, normalized);
}

if (require.main === module) {
  main(process.argv.slice(2));
}

module.exports = {
  GOOSE_COMPATIBILITY_NOTE,
  GOOSE_SERVERS_URL,
  GOOSE_SKILLS_MANIFEST_URL,
  convertGooseCommand,
  convertGooseText,
  normalizeMcpServer,
  normalizeServersCatalog,
  normalizeSkill,
  normalizeSkillsManifest,
  stableStringify,
};
