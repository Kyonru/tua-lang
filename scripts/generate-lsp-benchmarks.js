"use strict";

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const repoRoot = path.resolve(__dirname, "..");
const defaultOutputRoot = path.join(repoRoot, "target", "tua-benchmarks", "lsp-representative");
const fixtureDefinitions = [
  { name: "single-1000", kind: "single", lineCount: 1000 },
  { name: "single-5000", kind: "single", lineCount: 5000 },
  { name: "project-100", kind: "project", fileCount: 100 },
  { name: "project-1000", kind: "project", fileCount: 1000 },
  { name: "graph-chain-256", kind: "graph", shape: "chain", depth: 256 },
  { name: "graph-diamond-256", kind: "graph", shape: "diamond", width: 16 },
  { name: "graph-cycles-64", kind: "graph", shape: "cycles", width: 32 },
];

function generateRepresentativeFixtures(outputRoot = defaultOutputRoot, selectedNames = null) {
  const selected = selectedNames ? new Set(selectedNames) : null;
  const definitions = fixtureDefinitions.filter((fixture) => !selected || selected.has(fixture.name));

  if (selected && definitions.length !== selected.size) {
    const known = new Set(fixtureDefinitions.map((fixture) => fixture.name));
    const unknown = [...selected].filter((name) => !known.has(name));

    throw new Error(`unknown representative fixture(s): ${unknown.join(", ")}`);
  }

  fs.mkdirSync(outputRoot, { recursive: true });

  const fixtures = definitions.map((definition) => generateFixture(outputRoot, definition));
  const manifest = {
    schemaVersion: 1,
    fixtures,
  };

  fs.writeFileSync(
    path.join(outputRoot, "manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
    "utf8",
  );

  return manifest;
}

function generateFixture(outputRoot, definition) {
  const root = path.join(outputRoot, definition.name);
  const sourceRoot = path.join(root, "src");

  fs.rmSync(root, { force: true, recursive: true });
  fs.mkdirSync(sourceRoot, { recursive: true });
  fs.writeFileSync(path.join(root, "tua.toml"), fixtureConfig(), "utf8");

  if (definition.kind === "single") {
    fs.writeFileSync(
      path.join(sourceRoot, "main.tua"),
      singleFileSource(definition.lineCount),
      "utf8",
    );
  } else if (definition.kind === "project") {
    writeProjectSource(sourceRoot, definition.fileCount);
  } else {
    writeGraphProjectSource(sourceRoot, definition);
  }

  const sourceFiles = collectSourceFiles(sourceRoot);
  const sourceLineCount = sourceFiles.reduce((sum, file) => sum + lineCount(file.text), 0);

  if (definition.lineCount && sourceLineCount !== definition.lineCount) {
    throw new Error(
      `${definition.name}: expected ${definition.lineCount} source lines, generated ${sourceLineCount}`,
    );
  }
  if (definition.fileCount && sourceFiles.length !== definition.fileCount) {
    throw new Error(
      `${definition.name}: expected ${definition.fileCount} source files, generated ${sourceFiles.length}`,
    );
  }

  return {
    name: definition.name,
    kind: definition.kind,
    ...(definition.shape ? { graphShape: definition.shape } : {}),
    root: path.relative(repoRoot, root).replaceAll(path.sep, "/"),
    sourceFileCount: sourceFiles.length,
    sourceLineCount,
    contentHash: fixtureHash(sourceFiles),
  };
}

function writeGraphProjectSource(sourceRoot, definition) {
  if (definition.shape === "chain") {
    writeChainGraph(sourceRoot, definition.depth);
  } else if (definition.shape === "diamond") {
    writeDiamondGraph(sourceRoot, definition.width);
  } else if (definition.shape === "cycles") {
    writeCycleGraph(sourceRoot, definition.width);
  } else {
    throw new Error(`unknown graph shape: ${definition.shape}`);
  }

  fs.writeFileSync(path.join(sourceRoot, "main.tua"), graphMainSource(definition.shape), "utf8");
}

function writeChainGraph(sourceRoot, depth) {
  const graphRoot = path.join(sourceRoot, "graph");

  fs.mkdirSync(graphRoot, { recursive: true });
  for (let index = 0; index < depth; index += 1) {
    const current = String(index).padStart(3, "0");
    const next = String(index + 1).padStart(3, "0");
    fs.writeFileSync(
      path.join(graphRoot, `layer_${current}.tua`),
      [
        `-- Generated runtime chain layer ${current}.`,
        `local Next = require("graph.layer_${next}")`,
        "return { value = Next.value }",
        "",
      ].join("\n"),
      "utf8",
    );
  }
  fs.writeFileSync(
    path.join(graphRoot, `layer_${String(depth).padStart(3, "0")}.tua`),
    "-- Generated runtime chain leaf.\nreturn { value = 42 }\n",
    "utf8",
  );
}

function writeDiamondGraph(sourceRoot, width) {
  const graphRoot = path.join(sourceRoot, "graph");
  const leafRoot = path.join(graphRoot, "leaves");
  const branchRoot = path.join(graphRoot, "branches");

  fs.mkdirSync(leafRoot, { recursive: true });
  fs.mkdirSync(branchRoot, { recursive: true });

  for (let leaf = 0; leaf < width; leaf += 1) {
    const suffix = String(leaf).padStart(2, "0");
    fs.writeFileSync(
      path.join(leafRoot, `leaf_${suffix}.tua`),
      `-- Generated shared leaf ${suffix}.\nreturn { value = ${leaf + 1} }\n`,
      "utf8",
    );
  }

  const rootRequires = [];
  const rootFields = [];
  for (let branch = 0; branch < width; branch += 1) {
    const branchSuffix = String(branch).padStart(2, "0");
    const requires = [];
    const sum = [];

    for (let leaf = 0; leaf < width; leaf += 1) {
      const leafSuffix = String(leaf).padStart(2, "0");
      requires.push(`local Leaf${leafSuffix} = require("graph.leaves.leaf_${leafSuffix}")`);
      sum.push(`Leaf${leafSuffix}.value`);
    }

    fs.writeFileSync(
      path.join(branchRoot, `branch_${branchSuffix}.tua`),
      [
        `-- Generated fan-in branch ${branchSuffix}.`,
        ...requires,
        `return { value = ${sum.join(" + ")} }`,
        "",
      ].join("\n"),
      "utf8",
    );
    rootRequires.push(
      `local Branch${branchSuffix} = require("graph.branches.branch_${branchSuffix}")`,
    );
    rootFields.push(`  branch_${branchSuffix} = Branch${branchSuffix}.value,`);
  }

  fs.writeFileSync(
    path.join(graphRoot, "root.tua"),
    ["-- Generated diamond root.", ...rootRequires, "return {", ...rootFields, "}", ""].join(
      "\n",
    ),
    "utf8",
  );
}

function writeCycleGraph(sourceRoot, width) {
  const runtimeRoot = path.join(sourceRoot, "graph", "runtime");
  const typeRoot = path.join(sourceRoot, "graph", "types");

  fs.mkdirSync(runtimeRoot, { recursive: true });
  fs.mkdirSync(typeRoot, { recursive: true });

  for (let index = 0; index < width; index += 1) {
    const current = String(index).padStart(2, "0");
    const next = String((index + 1) % width).padStart(2, "0");
    fs.writeFileSync(
      path.join(runtimeRoot, `node_${current}.tua`),
      [
        `-- Generated runtime cycle node ${current}.`,
        `local Next = require("graph.runtime.node_${next}")`,
        `return { index = ${index}, next = Next }`,
        "",
      ].join("\n"),
      "utf8",
    );
    fs.writeFileSync(
      path.join(typeRoot, `node_${current}.d.tua`),
      [
        `-- Generated recursive structural type node ${current}.`,
        `import type Next from "./node_${next}"`,
        `export type Node${current} = { index: number, next: Next.Node${next} | nil }`,
        "",
      ].join("\n"),
      "utf8",
    );
  }
}

function graphMainSource(shape) {
  const graphEntry =
    shape === "chain"
      ? 'local Graph = require("graph.layer_000")'
      : shape === "diamond"
        ? 'local Graph = require("graph.root")'
        : 'local Graph = require("graph.runtime.node_00")';

  return [
    `-- Generated representative ${shape} dependency-graph benchmark.`,
    graphEntry,
    "type ProfileRecord = { value: number, label: string, active: boolean }",
    'local profile_primary: ProfileRecord = { value = 1, label = "primary", active = true }',
    "local profile_completion_probe = profile_primary.label",
    "local profile_hover_probe = profile_primary.value",
    "local profile_definition_probe = profile_primary",
    "return {",
    "  graph = Graph,",
    "  completion = profile_completion_probe,",
    "  hover = profile_hover_probe,",
    "  definition = profile_definition_probe,",
    "}",
    "",
  ].join("\n");
}

function fixtureConfig() {
  return [
    'source = "src"',
    'out = "build"',
    'target = "love11-luajit"',
    'extension = "tua"',
    "emit_maps = false",
    "",
  ].join("\n");
}

function singleFileSource(targetLines) {
  const header = [
    "-- Generated representative single-file LSP benchmark.",
    "type ProfileRecord = {",
    "  value: number,",
    "  label: string,",
    "  active: boolean,",
    "}",
    "local function make_profile_record(index: number): ProfileRecord",
    "  return { value = index, label = tostring(index), active = index % 2 == 0 }",
    "end",
  ];
  const tail = [
    "local profile_primary: ProfileRecord = make_profile_record(1)",
    "local profile_completion_probe = profile_primary.label",
    "local profile_hover_probe = profile_primary.value",
    "local profile_definition_probe = profile_primary",
    "return {",
    "  completion = profile_completion_probe,",
    "  hover = profile_hover_probe,",
    "  definition = profile_definition_probe,",
    "}",
  ];
  const fillerCount = targetLines - header.length - tail.length;

  if (fillerCount < 0) {
    throw new Error(`single-file target is too small: ${targetLines}`);
  }

  const declarationStride = fillerCount > 1000 ? Math.ceil(fillerCount / 1000) : 1;
  const filler = Array.from({ length: fillerCount }, (_, index) => {
    const suffix = String(index + 1).padStart(5, "0");

    if (index % declarationStride === 0) {
      return `local profile_value_${suffix}: number = ${index + 1}`;
    }

    return `-- Representative source-volume line ${suffix}.`;
  });

  return `${[...header, ...filler, ...tail].join("\n")}\n`;
}

function writeProjectSource(sourceRoot, fileCount) {
  const moduleCount = fileCount - 1;
  const moduleRoot = path.join(sourceRoot, "modules");

  fs.mkdirSync(moduleRoot, { recursive: true });

  const requires = [];
  for (let index = 1; index <= moduleCount; index += 1) {
    const suffix = String(index).padStart(4, "0");
    const moduleName = `Module${suffix}`;

    requires.push(`local ${moduleName} = require("modules.module_${suffix}")`);
    fs.writeFileSync(
      path.join(moduleRoot, `module_${suffix}.tua`),
      projectModuleSource(index, suffix),
      "utf8",
    );
  }

  const main = [
    "-- Generated representative multi-file LSP benchmark.",
    ...requires,
    "type ProfileRecord = {",
    "  value: number,",
    "  label: string,",
    "  active: boolean,",
    "}",
    'local profile_primary: ProfileRecord = { value = 1, label = "primary", active = true }',
    "local profile_completion_probe = profile_primary.label",
    "local profile_hover_probe = profile_primary.value",
    "local profile_definition_probe = profile_primary",
    "return {",
    "  completion = profile_completion_probe,",
    "  hover = profile_hover_probe,",
    "  definition = profile_definition_probe,",
    "}",
  ];

  fs.writeFileSync(path.join(sourceRoot, "main.tua"), `${main.join("\n")}\n`, "utf8");
}

function projectModuleSource(index, suffix) {
  return [
    `-- Generated benchmark module ${suffix}.`,
    "local record = {",
    `  value = ${index},`,
    `  label = "module_${suffix}",`,
    `  active = ${index % 2 === 0 ? "true" : "false"},`,
    "}",
    "local function increment(delta: number): number",
    "  return record.value + delta",
    "end",
    "return {",
    "  record = record,",
    "  increment = increment,",
    "}",
    "",
  ].join("\n");
}

function collectSourceFiles(sourceRoot) {
  const files = [];

  walk(sourceRoot, (filename) => {
    if (!filename.endsWith(".tua") && !filename.endsWith(".lua")) {
      return;
    }

    files.push({
      path: path.relative(sourceRoot, filename).replaceAll(path.sep, "/"),
      text: fs.readFileSync(filename, "utf8"),
    });
  });

  return files.sort((left, right) => left.path.localeCompare(right.path));
}

function fixtureHash(files) {
  const hash = crypto.createHash("sha256");

  for (const file of files) {
    hash.update(file.path);
    hash.update("\0");
    hash.update(file.text);
    hash.update("\0");
  }

  return hash.digest("hex");
}

function lineCount(text) {
  return text.endsWith("\n") ? text.split("\n").length - 1 : text.split("\n").length;
}

function walk(directory, visit) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const filename = path.join(directory, entry.name);

    if (entry.isDirectory()) {
      walk(filename, visit);
    } else {
      visit(filename);
    }
  }
}

if (require.main === module) {
  const outputRoot = path.resolve(process.argv[2] || defaultOutputRoot);
  const selectedNames = process.argv[3]
    ? process.argv[3].split(",").map((name) => name.trim()).filter(Boolean)
    : null;
  const manifest = generateRepresentativeFixtures(outputRoot, selectedNames);

  for (const fixture of manifest.fixtures) {
    console.log(
      `${fixture.name}: files=${fixture.sourceFileCount} lines=${fixture.sourceLineCount} ` +
        `sha256=${fixture.contentHash}`,
    );
  }
  console.log(`wrote ${path.join(outputRoot, "manifest.json")}`);
}

module.exports = {
  fixtureDefinitions,
  generateRepresentativeFixtures,
};
