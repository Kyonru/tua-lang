"use strict";

const assert = require("node:assert/strict");
const test = require("node:test");
const Module = require("node:module");
const fs = require("node:fs/promises");
const { exactPattern, planRuns, testArguments } = require("../../src/testing/model");
const { runProcess } = require("../../src/testing/process");

function collection(parent) {
  const map = new Map();
  return {
    get: id => map.get(id),
    forEach: fn => map.forEach(fn),
    add(item) { item.parent = parent; map.set(item.id, item); },
    replace(items) { map.clear(); for (const item of items) this.add(item); },
    get size() { return map.size; },
  };
}
function item(id, label = id, uri) { const value = { id, label, uri }; value.children = collection(value); return value; }
function cancellation() {
  const listeners = new Set();
  return { isCancellationRequested: false,
    onCancellationRequested(listener) { listeners.add(listener); return { dispose() { listeners.delete(listener); } }; },
    cancel() { this.isCancellationRequested = true; [...listeners].forEach(listener => listener()); },
  };
}

test("test selection deduplicates ancestors and respects excluded files/cases", () => {
  const project = item("project"), file = item("file"), one = item("one"), two = item("two");
  project.children.add(file); file.children.add(one); file.children.add(two);
  const data = new WeakMap([[project, { root: "/project" }], [file, { selector: "spec.tua" }], [one, { caseName: "one" }], [two, { caseName: "two" }]]);
  assert.deepEqual(planRuns([project], { include: [project, one] }, data).map(job => job.item.id), ["project"]);
  assert.deepEqual(planRuns([project], { exclude: [two] }, data).map(job => job.item.id), ["one"]);
  assert.deepEqual(planRuns([project], { include: [one], exclude: [file] }, data), []);
  assert.equal(exactPattern("math (1) + 2? %"), "^math %(1%) %+ 2%? %%$");
  assert.deepEqual(testArguments({ selector: "a.tua", suiteName: "math [x]." }, "/tmp/report.json", { get: () => "" }), ["test", "a.tua", "--filter", "^math %[x%]%. ", "--report", "/tmp/report.json"]);
  assert.deepEqual(testArguments({ selector: "a spec.tua", caseName: "[x]" }, "/tmp/report.json", { get: () => "" }), ["test", "a spec.tua", "--filter", "^%[x%]$", "--report", "/tmp/report.json"]);
});

test("suite selections deduplicate descendants and preserve excluded nested cases", () => {
  const project = item("project"), file = item("file"), suite = item("suite"), nested = item("nested");
  const one = item("one"), two = item("two");
  project.children.add(file); file.children.add(suite); suite.children.add(nested);
  nested.children.add(one); nested.children.add(two);
  const data = new WeakMap([[project, {}], [file, { selector: "spec.tua" }],
    [suite, { suiteName: "math" }], [nested, { suiteName: "math positive" }],
    [one, { caseName: "math positive one" }], [two, { caseName: "math positive two" }]]);
  assert.deepEqual(planRuns([project], { include: [suite, one] }, data).map(job => job.item.id), ["suite"]);
  assert.deepEqual(planRuns([project], { include: [suite], exclude: [two] }, data).map(job => job.item.id), ["one"]);
  assert.deepEqual(planRuns([project], { include: [one], exclude: [suite] }, data), []);
});

test("test process runner preserves Unicode and supports cancellation without a shell", async () => {
  const passed = await runProcess(process.execPath, ["-e", "console.log(process.argv[1])", "日本語 $(no shell)"], { cwd: process.cwd() });
  assert.equal(passed.stdout.trim(), "日本語 $(no shell)");
  const token = cancellation();
  const pending = runProcess(process.execPath, ["-e", "setTimeout(()=>{}, 10000)"], { cwd: process.cwd(), token });
  token.cancel();
  assert.equal((await pending).cancelled, true);
});

test("Opening a test document discovers gutter cases and suites without running tests", async () => {
  const root = "/tmp/tua-testing-project";
  const source = `${root}/tests/math_spec.tua`;
  const commands = new Map(), calls = [], runs = [], watchers = [];
  let profile, coverageProfile, openDocument;
  const controller = {
    items: collection(),
    createTestItem: item,
    createRunProfile(_name, kind, handler) { const value = { handler, dispose() {} }; if (kind === 1) profile = value; else coverageProfile = value; return value; },
    createTestRun() {
      const events = [];
      const run = { events, addCoverage(value) { events.push(["coverage", value]); }, end() { events.push(["end"]); }, appendOutput(value) { events.push(["output", value]); } };
      for (const status of ["enqueued", "started", "passed", "failed", "errored", "skipped"]) run[status] = (item, message) => events.push([status, item.id, message]);
      runs.push(run); return run;
    },
    dispose() {},
  };
  const vscode = {
    tests: { createTestController: () => controller },
    TestRunProfileKind: { Run: 1, Coverage: 3 },
    TestMessage: class { constructor(message) { this.message = message; } },
    TestRunRequest: class { constructor(include, exclude, profile, continuous) { Object.assign(this, { include, exclude, profile, continuous }); } },
    Range: class { constructor(...values) { this.values = values; } },
    Location: class { constructor(uri, range) { Object.assign(this, { uri, range }); } },
    Uri: { file: fsPath => ({ fsPath, toString: () => `file://${fsPath}` }) },
    RelativePattern: class {},
    Disposable: { from: (...values) => ({ dispose() { values.forEach(value => value.dispose()); } }) },
    commands: { registerCommand(id, handler) { commands.set(id, handler); return { dispose() {} }; } },
    window: { showWarningMessage() {}, showErrorMessage(message) { throw new Error(message); } },
    workspace: {
      isTrusted: true,
      workspaceFolders: [{ uri: { fsPath: root } }],
      findFiles: async () => [{ fsPath: `${root}/tua.toml` }],
      onDidOpenTextDocument(handler) { openDocument = handler; return { dispose() {} }; },
      getConfiguration: () => ({ get: () => "" }),
      createFileSystemWatcher() {
        const watcher = { dispose() {}, onDidCreate() { return { dispose() {} }; }, onDidDelete() { return { dispose() {} }; }, onDidChange(handler) { this.change = handler; return { dispose() {} }; } };
        watchers.push(watcher); return watcher;
      },
    },
  };
  vscode.FileCoverage = class { constructor(uri, statementCoverage) { Object.assign(this, { uri, statementCoverage }); } };
  vscode.StatementCoverage = class { constructor(executed, location) { Object.assign(this, { executed, location }); } };
  vscode.Position = class { constructor(line, character) { Object.assign(this, { line, character }); } };
  const original = Module._load;
  Module._load = function (id, ...args) { return id === "vscode" ? vscode : original.call(this, id, ...args); };
  const { registerTesting } = require("../../src/testing");
  Module._load = original;
  let outcome = "passed";
  const execute = async (_command, args) => {
    calls.push(args);
    if (args.includes("--list")) return { code: 0, stdout: JSON.stringify({ version: 1, root: `/canonical${root}`, source_roots: [`/canonical${root}/src`, `/canonical${root}/tests`], output_root: `/canonical${root}/generated`, files: [{ path: `/canonical${source}`, selector: "math_spec.tua", suites: [{ name: "math", full_name: "math", line: 1, column: 1 }], cases: [{ full_name: "math works [1]", suite: "math", line: 2, column: 1 }] }] }) };
    if (args.includes("--coverage")) await fs.writeFile(args[args.indexOf("--coverage") + 1], JSON.stringify({ version: 1, completed: true, files: [{path: source, lines: [{line: 2, hits: 3}, {line: 3, hits: 0}]}] }));
    const report = args[args.indexOf("--report") + 1];
    await fs.writeFile(report, JSON.stringify({ version: 1, completed: true, success: outcome === "passed", tests: [{ name: "math works [1]", status: outcome, message: "assertion", file: `/canonical${source}`, line: 3 }] }));
    return { code: outcome === "passed" ? 0 : 1, stdout: "test output\n", stderr: "" };
  };
  const disposable = registerTesting({ getServerCommand: () => "tua", execute });
  try {
    assert.equal(calls.length, 0);
    vscode.workspace.isTrusted = false;
    await controller.resolveHandler();
    assert.equal(calls.length, 0);
    const document = { uri: { fsPath: source, scheme: "file" }, languageId: "tua" };
    openDocument(document);
    assert.equal(calls.length, 0, "untrusted editors must not trigger discovery");
    vscode.workspace.isTrusted = true;
    openDocument({ ...document, uri: { fsPath: "/outside/spec.tua", scheme: "file" } });
    assert.equal(calls.length, 0, "documents outside the workspace must not trigger discovery");
    openDocument(document);
    await new Promise(resolve => setTimeout(resolve, 350));
    assert.equal(calls.length, 1, "opening a spec should discover without opening Testing");
    assert.equal(runs.length, 0, "automatic discovery must not execute tests");
    const project = controller.items.get(root);
    const file = project.children.get(`file://${source}`);
    const suite = file.children.get(`file://${source}::suite::math`);
    const selected = suite.children.get(`file://${source}::case::math works [1]`);
    assert.deepEqual(suite.range.values, [0, 0, 0, 0]);
    assert.deepEqual(selected.range.values, [1, 0, 1, 0]);
    await profile.handler({ include: [selected] }, cancellation());
    assert.ok(calls[1].includes("^math works %[1%]$"));
    assert.ok(runs[0].events.some(event => event[0] === "passed" && event[1] === selected.id));
    outcome = "failed";
    await profile.handler({ include: [selected] }, cancellation());
    const failure = runs[1].events.find(event => event[0] === "failed" && event[2]?.location);
    assert.equal(failure[2].location.uri.fsPath, source);
    assert.deepEqual(failure[2].location.range.values, [2, 0, 2, 0]);
    await coverageProfile.handler({ include: [selected] }, cancellation());
    const coverage = runs[2].events.find(event => event[0] === "coverage")[1];
    assert.deepEqual(coverage.statementCoverage, { covered: 1, total: 2 });
    const details = coverageProfile.loadDetailedCoverage(runs[2], coverage);
    assert.equal(details[0].location.line, 1);
    assert.equal(details[0].executed, 3);
    assert.equal(details[1].executed, 0);
    assert.ok(runs[2].events.some(event => event[0] === "failed"));
    outcome = "passed";
    const baseline = runs.length;
    watchers[0].change({ fsPath: `${root}/src/library.tua` });
    await new Promise(resolve => setTimeout(resolve, 350));
    assert.equal(runs.length, baseline, "refresh alone must not execute specs");
    const token = cancellation();
    await profile.handler({ include: [selected], continuous: true }, token);
    watchers[0].change({ fsPath: `${root}/src/library.tua` });
    await new Promise(resolve => setTimeout(resolve, 350));
    assert.equal(runs.length, baseline + 2);
    token.cancel();
    watchers[0].change({ fsPath: `${root}/generated/library.lua` });
    watchers[0].change({ fsPath: `${root}/.tua/test-1/spec.lua` });
    await new Promise(resolve => setTimeout(resolve, 350));
    assert.equal(runs.length, baseline + 2);
    await profile.handler({ include: [suite] }, cancellation());
    assert.ok(calls.at(-1).includes("^math "), "suite gutter runs include all descendants by prefix");
    assert.equal(suite.children.size, 1, "run results should reuse the nested case item");
    assert.equal(file.children.size, 1, "run results must not duplicate nested cases at file level");
  } finally { disposable.dispose(); }
});

test("coverage merges selected runs and rejects incomplete data", () => {
  const { mergeCoverage } = require("../../src/testing/coverage");
  const files = new Map();
  const report = hits => ({version: 1, completed: true, files: [{path: "/source.tua", lines: [{line: 2, hits}, {line: 4, hits: 0}]}]});
  mergeCoverage(files, report(3)); mergeCoverage(files, report(2));
  assert.deepEqual([...files.get("/source.tua")], [[2, 5], [4, 0]]);
  assert.throws(() => mergeCoverage(files, {version: 1, completed: false, files: []}), /did not complete/);
  assert.throws(() => mergeCoverage(new Map(), report(-1)), /Invalid coverage line/);
});

test("test discovery timeout terminates a child that ignores SIGTERM", { timeout: 6000, skip: process.platform === "win32" }, async () => {
  const started = Date.now();
  await assert.rejects(runProcess(process.execPath, ["-e",
    "process.on('SIGTERM', () => {}); setInterval(() => {}, 1000);"],
    { timeoutMs: 300 }), /Test discovery timed out/);
  assert.ok(Date.now() - started < 5000, "timeout must release the testing queue");
});

test("test logs may be truncated without replacing the backend exit status", { timeout: 6000 }, async () => {
  const result = await runProcess(process.execPath, ["-e",
    "process.stdout.write('x'.repeat(17 * 1024 * 1024)); process.exitCode = 7;"],
    { truncateOutput: true });
  assert.equal(result.code, 7);
  assert.equal(result.cancelled, false);
  assert.ok(result.stdout.length <= 16 * 1024 * 1024 + 128);
  assert.match(result.stdout, /Test output truncated/);
});
