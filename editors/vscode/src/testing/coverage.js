"use strict";

// Merge independent selected runs by source line. Each CLI report has already
// collapsed compiler-generated lines onto their original source locations.
function mergeCoverage(files, report) {
  if (report.version !== 1 || !report.completed || !Array.isArray(report.files)) {
    throw new Error("Coverage did not complete. Install LuaCov for the Busted interpreter.");
  }
  for (const file of report.files) {
    if (typeof file.path !== "string" || !Array.isArray(file.lines)) throw new Error("Invalid coverage file");
    let lines = files.get(file.path);
    if (!lines) files.set(file.path, lines = new Map());
    for (const { line, hits } of file.lines) {
      if (!Number.isSafeInteger(line) || line < 1 || !Number.isSafeInteger(hits) || hits < 0) throw new Error("Invalid coverage line");
      lines.set(line, (lines.get(line) || 0) + hits);
    }
  }
}

function publishCoverage(vscode, run, files, details) {
  for (const [path, lines] of files) {
    const covered = [...lines.values()].filter(hits => hits > 0).length;
    const coverage = new vscode.FileCoverage(vscode.Uri.file(path), { covered, total: lines.size });
    details.set(coverage, [...lines].sort(([a], [b]) => a - b).map(([line, hits]) =>
      new vscode.StatementCoverage(hits, new vscode.Position(line - 1, 0))));
    run.addCoverage(coverage);
  }
}

module.exports = { mergeCoverage, publishCoverage };
