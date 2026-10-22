"use strict";

const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const schemaPath = path.join(repoRoot, "config", "schema.json");
const packagePath = path.join(repoRoot, "editors", "vscode", "package.json");
const docsPath = path.join(repoRoot, "docs", "configuration.md");
const templateDir = path.join(repoRoot, "config", "templates");
const defaultTemplatePath = path.join(templateDir, "default.toml");
const minimalTemplatePath = path.join(templateDir, "minimal.toml");
const checkOnly = process.argv.includes("--check");

const schema = JSON.parse(fs.readFileSync(schemaPath, "utf8"));
validateSchema(schema);

const manifest = JSON.parse(fs.readFileSync(packagePath, "utf8"));
const properties = manifest.contributes?.configuration?.properties;

if (!properties) {
  throw new Error("editors/vscode/package.json is missing configuration properties");
}

for (const setting of schema.settings.filter((setting) => setting.editor)) {
  if (!properties[setting.editor]) {
    throw new Error(`VS Code manifest is missing schema-backed setting ${setting.editor}`);
  }

  properties[setting.editor] = editorProperty(setting);
}

const generatedPackage = `${JSON.stringify(manifest, null, 2)}\n`;
const currentDocs = fs.readFileSync(docsPath, "utf8");
const generatedDocs = replaceGeneratedBlock(currentDocs, docsReference(schema.settings));
const generatedDefaultTemplate = defaultTemplate(schema.settings);
const generatedMinimalTemplate = minimalTemplate(schema.settings);

checkOrWrite(packagePath, generatedPackage);
checkOrWrite(docsPath, generatedDocs);
checkOrWrite(defaultTemplatePath, generatedDefaultTemplate);
checkOrWrite(minimalTemplatePath, generatedMinimalTemplate);

if (checkOnly) {
  console.log(`config schema is synchronized (${schema.settings.length} TOML settings)`);
} else {
  console.log(`generated config metadata (${schema.settings.length} TOML settings)`);
}

function validateSchema(value) {
  if (value.schemaVersion !== 1 || !Array.isArray(value.settings)) {
    throw new Error("config/schema.json must contain schemaVersion 1 and a settings array");
  }

  const paths = new Set();
  const editors = new Set();

  for (const setting of value.settings) {
    if (
      typeof setting.path !== "string" ||
      typeof setting.type !== "string" ||
      typeof setting.description !== "string" ||
      !Object.hasOwn(setting, "default")
    ) {
      throw new Error(`invalid config schema setting: ${JSON.stringify(setting)}`);
    }
    if (paths.has(setting.path)) {
      throw new Error(`duplicate TOML config path: ${setting.path}`);
    }
    paths.add(setting.path);

    if (setting.editor) {
      if (editors.has(setting.editor)) {
        throw new Error(`duplicate VS Code config path: ${setting.editor}`);
      }
      editors.add(setting.editor);
    }
  }
}

function editorProperty(setting) {
  const property = {
    type: [editorType(setting.type), "null"],
    default: null,
  };

  if (setting.minimum !== undefined) {
    property.minimum = setting.minimum;
  }
  if (setting.enum) {
    property.enum = [...setting.enum, null];
  }
  if (setting.type === "string-array") {
    property.items = { type: "string" };
  }

  property.description = setting.editorDescription || setting.description;
  return property;
}

function editorType(type) {
  switch (type) {
    case "boolean":
      return "boolean";
    case "integer":
      return "number";
    case "string":
      return "string";
    case "string-array":
      return "array";
    default:
      throw new Error(`unsupported config schema type: ${type}`);
  }
}

function docsReference(settings) {
  const rows = settings.map((setting) => {
    const editor = setting.editor ? `\`${setting.editor}\`` : "-";
    return `| \`${setting.path}\` | ${setting.type} | \`${formatDefault(setting.default)}\` | ${editor} |`;
  });

  return [
    "<!-- tua-config-schema:start -->",
    "| TOML path | Type | Default | VS Code override |",
    "| --- | --- | --- | --- |",
    ...rows,
    "<!-- tua-config-schema:end -->",
  ].join("\n");
}

function defaultTemplate(settings) {
  const root = [];
  const sections = new Map();

  for (const setting of settings) {
    const parts = setting.path.split(".");
    const line = `${parts.at(-1)} = ${tomlValue(setting.default)}`;

    if (parts.length === 1) {
      root.push(line);
      continue;
    }

    const section = parts.slice(0, -1).join(".");
    if (!sections.has(section)) {
      sections.set(section, []);
    }
    sections.get(section).push(line);
  }

  const chunks = [root.join("\n")];

  for (const [section, lines] of sections) {
    chunks.push(`[${section}]\n${lines.join("\n")}`);
  }

  return `${chunks.join("\n\n")}\n`;
}

function minimalTemplate(settings) {
  const paths = new Set(["source", "out", "main", "target", "extension", "emit_maps"]);
  const lines = settings
    .filter((setting) => paths.has(setting.path))
    .map((setting) => `${setting.path} = ${tomlValue(setting.default)}`);

  return `${lines.join("\n")}\n`;
}

function tomlValue(value) {
  if (typeof value === "string") {
    return JSON.stringify(value);
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(tomlValue).join(", ")}]`;
  }
  if (value && typeof value === "object" && Object.keys(value).length === 0) {
    return "{}";
  }

  throw new Error(`unsupported TOML template value: ${JSON.stringify(value)}`);
}

function formatDefault(value) {
  return JSON.stringify(value).replaceAll("|", "\\|");
}

function replaceGeneratedBlock(contents, generated) {
  const start = "<!-- tua-config-schema:start -->";
  const end = "<!-- tua-config-schema:end -->";
  const startIndex = contents.indexOf(start);
  const endIndex = contents.indexOf(end);

  if (startIndex < 0 || endIndex < startIndex) {
    throw new Error("docs/configuration.md is missing config schema generation markers");
  }

  return `${contents.slice(0, startIndex)}${generated}${contents.slice(endIndex + end.length)}`;
}

function checkOrWrite(file, generated) {
  let current = null;

  try {
    current = fs.readFileSync(file, "utf8");
  } catch (error) {
    if (error.code !== "ENOENT") {
      throw error;
    }
  }

  if (current === generated) {
    return;
  }
  if (checkOnly) {
    console.error(`${path.relative(repoRoot, file)} is out of date; run make generate-config-schema`);
    process.exitCode = 1;
    return;
  }

  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, generated, "utf8");
}
