"use strict";

const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const skillsRoot = path.join(root, "skills");
const aliasesRoot = path.join(root, ".codex", "skills");
const failures = [];

if (!fs.existsSync(path.join(root, "AGENTS.md"))) {
  failures.push("missing root AGENTS.md");
}

const skills = fs.readdirSync(skillsRoot, { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && entry.name.startsWith("tua-")
    && fs.existsSync(path.join(skillsRoot, entry.name, "SKILL.md")))
  .map((entry) => entry.name);

for (const name of skills) {
  const skillPath = path.join(skillsRoot, name);
  const source = fs.readFileSync(path.join(skillPath, "SKILL.md"), "utf8");
  const declared = source.match(/^---\r?\nname: ([^\r\n]+)\r?\n/);
  if (declared?.[1] !== name) {
    failures.push(`${name}: SKILL.md frontmatter name must match directory`);
  }
  if (!fs.existsSync(path.join(skillPath, "agents", "openai.yaml"))) {
    failures.push(`${name}: missing agents/openai.yaml`);
  }
  const aliasPath = path.join(aliasesRoot, name);
  if (!fs.existsSync(aliasPath) || !fs.lstatSync(aliasPath).isSymbolicLink()
    || fs.realpathSync(aliasPath) !== fs.realpathSync(skillPath)) {
    failures.push(`${name}: missing or incorrect .codex/skills alias`);
  }
}

for (const entry of fs.readdirSync(aliasesRoot)) {
  if (entry.startsWith("tua-") && !skills.includes(entry)) {
    failures.push(`${entry}: alias has no matching skill`);
  }
}

if (failures.length) {
  for (const failure of failures) console.error(failure);
  process.exit(1);
}
console.log(`AI tooling: AGENTS.md and ${skills.length} skills/aliases are consistent`);
