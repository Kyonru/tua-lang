"use strict";

function exactPattern(name) {
  const magic = new Set(["^", "$", "(", ")", "%", ".", "[", "]", "*", "+", "-", "?"]);
  return `^${[...name].map(char => magic.has(char) ? `%${char}` : char).join("")}$`;
}

function descendants(item) {
  const result = [];
  item.children.forEach(child => { result.push(child, ...descendants(child)); });
  return result;
}

function planRuns(roots, request, data) {
  const excludes = new Set((request.exclude || []).map(item => item.id));
  const included = request.include || roots;
  const selected = new Set(included.map(item => item.id));
  const jobs = [];
  const visit = item => {
    if (excludes.has(item.id)) return;
    const metadata = data.get(item);
    if (!metadata) return;
    const hasExcluded = descendants(item).some(child => excludes.has(child.id));
    if ((hasExcluded || metadata.expandOnly) && metadata.caseName === undefined) {
      item.children.forEach(visit);
    } else {
      jobs.push({ item, ...metadata });
    }
  };
  for (const item of included) {
    let parent = item.parent;
    while (parent && !selected.has(parent.id) && !excludes.has(parent.id)) parent = parent.parent;
    if (!parent) visit(item);
  }
  return jobs;
}

function testArguments(job, reportPath, settings, coveragePath) {
  const args = ["test"];
  if (job.selector) args.push(job.selector);
  if (job.caseName !== undefined) args.push("--filter", exactPattern(job.caseName));
  else if (job.suiteName !== undefined) args.push("--filter", `${exactPattern(job.suiteName).slice(0, -1)} `);
  args.push("--report", reportPath);
  if (coveragePath) args.push("--coverage", coveragePath);
  for (const [setting, flag] of [["test.bustedPath", "--busted"], ["test.luaPath", "--lua"]]) {
    const value = settings.get(setting, "").trim();
    if (value) args.push(flag, value);
  }
  return args;
}

module.exports = { exactPattern, descendants, planRuns, testArguments };
