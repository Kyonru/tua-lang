"use strict";

const path = require("path");
const {
  printRequestReport,
  profileLspProject,
  resolveCommand,
} = require("./lsp-profile-common");

const command = resolveCommand(process.argv[2] || "target/debug/tua");
const roots = (process.argv.slice(3).length ? process.argv.slice(3) : ["benchmarks/lsp/small"]).map((root) =>
  path.resolve(root),
);
const repeat = Number(process.env.TUA_PROFILE_REPEAT || "5");

(async () => {
  for (const root of roots) {
    const result = await profileLspProject({
      command,
      root,
      repeat,
      sourceRoot: path.join(root, "src"),
    });

    printRequestReport(root, result.timings, result.stats);

    if (process.env.TUA_PROFILE && result.stderr.trim()) {
      console.error(`\n# ${path.basename(root)} trace`);
      console.error(result.stderr.trim());
    }
  }
})().catch((error) => {
  console.error(error.stack || error.message);
  process.exit(1);
});
