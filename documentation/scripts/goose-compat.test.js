const { test, describe } = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const ts = require("typescript");
const {
  normalizeMcpServer,
  normalizeServersCatalog,
  normalizeSkillsManifest,
  isSupportedGooseSkill,
  convertGooseText,
  convertGooseCommand,
} = require("./goose-compat");

function loadBrowserConverter() {
  const source = fs.readFileSync(
    path.join(__dirname, "../src/utils/goose-compat.ts"),
    "utf8",
  );
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  const module = { exports: {} };
  new Function("module", "exports", "require", compiled)(
    module,
    module.exports,
    require,
  );
  return module.exports;
}

function jsonValue(value) {
  return JSON.parse(JSON.stringify(value));
}

describe("goose compatibility conversion", () => {
  test("converts Goose extension metadata to Gosling install metadata", () => {
    const server = normalizeMcpServer({
      id: "demo",
      name: "goose Demo",
      command: "goose mcp demo",
      installation_notes: "Install with goose session.",
      type: "streamable_http",
      environmentVariables: [{ name: "TOKEN", required: true }],
    });

    assert.strictEqual(server.name, "gosling Demo");
    assert.strictEqual(server.command, "gosling mcp demo");
    assert.strictEqual(server.installation_notes, "Install with gosling session.");
    assert.strictEqual(server.type, "streamable-http");
    assert.strictEqual(
      server.environmentVariables[0].description,
      "Required environment variable",
    );
    assert.strictEqual(server.sourceCatalog, "goose");
  });

  test("deduplicates and sorts servers by id", () => {
    const servers = normalizeServersCatalog([
      { id: "zeta", name: "Zeta" },
      { id: "alpha", name: "Alpha" },
      { id: "alpha", name: "Duplicate" },
    ]);

    assert.deepStrictEqual(
      servers.map((server) => server.id),
      ["alpha", "zeta"],
    );
    assert.strictEqual(servers[0].name, "Alpha");
  });

  test("converts Goose skill manifest provenance without marking it community", () => {
    const manifest = normalizeSkillsManifest({
      skills: [
        {
          id: "api-setup",
          name: "goose setup",
          author: "goose",
          repoUrl: "https://github.com/block/Agent-Skills",
          installCommand:
            "npx skills add https://github.com/block/Agent-Skills --skill api-setup",
        },
      ],
    });

    assert.strictEqual(manifest.count, 1);
    assert.strictEqual(manifest.skills[0].name, "gosling setup");
    assert.strictEqual(manifest.skills[0].author, "gosling via goose");
    assert.strictEqual(manifest.skills[0].isCommunity, false);
    assert.strictEqual(manifest.skills[0].sourceCatalog, "goose");
  });

  test("excludes inherited CI and review-policy skills", () => {
    const manifest = normalizeSkillsManifest({
      skills: [
        { id: "api-setup" },
        { id: "code-review" },
        { id: "testing-strategy" },
      ],
    });

    assert.deepStrictEqual(
      manifest.skills.map((skill) => skill.id),
      ["api-setup"],
    );
    assert.strictEqual(isSupportedGooseSkill({ id: "code-review" }), false);
    assert.strictEqual(isSupportedGooseSkill({ id: "testing-strategy" }), false);
    assert.strictEqual(isSupportedGooseSkill({ id: "frontend-design" }), true);
  });

  test("rewrites Goose branding regardless of source casing", () => {
    assert.strictEqual(convertGooseText("GOOSE session in progress"), "gosling session in progress");
    assert.strictEqual(convertGooseText("Goose mcp demo"), "gosling mcp demo");
    assert.strictEqual(convertGooseText("Visit goose://open"), "Visit gosling://open");
    assert.strictEqual(convertGooseCommand("Goose mcp start"), "gosling mcp start");
  });

  test("drops catalog entries with a missing id and warns instead of silently discarding them", () => {
    const originalWarn = console.warn;
    const warnings = [];
    console.warn = (...args) => warnings.push(args);

    try {
      const servers = normalizeServersCatalog([
        { name: "No ID Server" },
        { id: "ok", name: "OK Server" },
      ]);

      assert.strictEqual(servers.length, 1);
      assert.strictEqual(servers[0].id, "ok");
      assert.strictEqual(warnings.length, 1);
    } finally {
      console.warn = originalWarn;
    }
  });

  test("build-time and live catalog converters stay byte-equivalent", () => {
    const browser = loadBrowserConverter();
    const server = {
      id: "demo",
      name: "Goose Demo",
      description: "Use goose mcp for this Goose server",
      command: "goose mcp start",
      url: "https://example.com/mcp",
      type: "streamable_http",
      installation_notes: "Open goose://settings",
      environmentVariables: [{ name: "TOKEN", description: "Goose token", required: true }],
      headers: [{ name: "X-Tenant", required: false }],
      endorsed: true,
    };
    const skill = {
      id: "demo-skill",
      name: "Goose Skill",
      description: "Run goose session",
      author: "goose",
      version: "1.2.3",
      status: "experimental",
      tags: ["workflow"],
      sourceUrl: "https://example.com/source",
      repoUrl: "https://example.com/repo",
      content: "instructions",
      hasSupporting: true,
      supportingFiles: ["reference.md"],
      supportingFilesType: "directory",
      installMethod: "npx-multi",
      installCommand: "goose mcp install demo",
    };

    assert.deepStrictEqual(
      jsonValue(normalizeMcpServer(server)),
      jsonValue(browser.normalizeGooseServer(server)),
    );
    assert.deepStrictEqual(
      jsonValue(require("./goose-compat").normalizeSkill(skill)),
      jsonValue(browser.normalizeGooseSkill(skill)),
    );
  });
});
