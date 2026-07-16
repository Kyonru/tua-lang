"use strict";

const fs = require("fs");
const path = require("path");
const {
  printRequestReport,
  profileLspProject,
  resolveCommand,
  statsForValues,
  timingsByLabel,
} = require("./lsp-profile-common");

const command = resolveCommand(process.argv[2] || "target/debug/tua");
const roots = (process.argv.slice(3).length ? process.argv.slice(3) : ["benchmarks/lsp/small"]).map((root) =>
  path.resolve(root),
);
const repeat = Number(process.env.TUA_PROFILE_REPEAT || "5");
const outputPath = path.resolve(
  process.env.TUA_LSP_PROFILE_OUT || path.join("target", "tua-speed", "lsp-profile.json"),
);

(async () => {
  const projects = [];

  for (const root of roots) {
    const result = await profileLspProject({
      command,
      root,
      repeat,
      sourceRoot: path.join(root, "src"),
    });

    printRequestReport(root, result.timings, result.stats);
    projects.push({
      root,
      requestLatency: Object.fromEntries(
        [...timingsByLabel(result.timings)].map(([label, values]) => [label, statsForValues(values)]),
      ),
      cacheStats: result.stats,
    });

    if (process.env.TUA_PROFILE && result.stderr.trim()) {
      console.error(`\n# ${path.basename(root)} trace`);
      console.error(result.stderr.trim());
    }
  }

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(
    outputPath,
    `${JSON.stringify({ generatedAt: new Date().toISOString(), command, repeat, projects }, null, 2)}\n`,
    "utf8",
  );
  console.log(`wrote ${outputPath}`);
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
