"use strict";

const fs = require("node:fs/promises");
const os = require("node:os");
const path = require("node:path");
const vscode = require("vscode");
const { mergeCoverage, publishCoverage } = require("./coverage");
const { runProcess } = require("./process");
const { descendants, planRuns, testArguments } = require("./model");

function registerTesting({ getServerCommand, execute = runProcess }) {
  const controller = vscode.tests.createTestController("tua.busted", "Tua Unit Tests");
  const data = new WeakMap();
  const caseItems = new WeakMap();
  const coverageDetails = new WeakMap();
  const projectPaths = new WeakMap();
  const continuous = new Set();
  const cancellations = new Set();
  let watchedRoots = [], outputRoots = [], configPaths = [];
  const within = (file, root) => { const relative = path.relative(root, file); return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative)); };
  const invoke = (args, options) => execute(getServerCommand(), args, { ...options, token: {
    get isCancellationRequested() { return disposed || Boolean(options.token?.isCancellationRequested); },
    onCancellationRequested(listener) {
      cancellations.add(listener);
      const subscription = options.token?.onCancellationRequested(listener);
      return { dispose() { cancellations.delete(listener); subscription?.dispose(); } };
    },
  } });
  const watchers = [];
  let queue = Promise.resolve(), timer, initialized = false, disposed = false, refreshQueued = false;
  const enqueue = task => {
    const next = queue.then(() => disposed ? undefined : task());
    queue = next.catch(() => {});
    return next;
  };
  const roots = () => { const items = []; controller.items.forEach(item => items.push(item)); return items; };
  const children = item => { const result = []; item.children.forEach(child => result.push(child)); return result; };
  const sourceRange = (line = 1, column = 1) => new vscode.Range(Math.max(0, line - 1), Math.max(0, column - 1), Math.max(0, line - 1), Math.max(0, column - 1));
  const caseItem = (file, name, line, column, preserveRange = false, parent) => {
    const id = `${file.id}::case::${name}`;
    const known = caseItems.get(file) || new Map();
    caseItems.set(file, known);
    const item = known.get(id) || controller.createTestItem(id, name || "(unnamed test)", file.uri);
    if (!preserveRange || !item.range) item.range = sourceRange(line, column);
    known.set(id, item);
    (parent || item.parent || file).children.add(item);
    data.set(item, { ...data.get(file), caseName: name, expandOnly: false });
    return item;
  };
  async function refresh(token) {
    if (!vscode.workspace.isTrusted || disposed) return;
    initialized = true;
    const configs = await vscode.workspace.findFiles("**/tua.toml", "**/{.git,.tua,node_modules,build,dist}/**", 200, token);
    const projectRoots = [...new Set(configs.map(uri => path.dirname(uri.fsPath)))];
    const active = new Set();
    watchedRoots = []; outputRoots = []; configPaths = configs.map(uri => uri.fsPath);
    for (const root of projectRoots) {
      if (token?.isCancellationRequested) break;
      const id = root;
      active.add(id);
      const project = controller.items.get(id) || controller.createTestItem(id, path.basename(root), vscode.Uri.file(path.join(root, "tua.toml")));
      controller.items.add(project);
      data.set(project, { root, project });
      try {
        const result = await invoke(["test", "--list"], { cwd: root, token, timeoutMs: 60000 });
        if (result.cancelled) return;
        if (result.code !== 0) throw new Error(result.stderr || result.stdout);
        const inventory = JSON.parse(result.stdout);
        if (inventory.version !== 1 || !Array.isArray(inventory.files)) throw new Error("The selected Tua server does not support test discovery version 1");
        // CLI discovery canonicalizes roots; retain the URI spelling used by
        // the editor so native gutter locations also match symlinked workspaces.
        const editorPath = file => inventory.root && within(file, inventory.root)
          ? path.join(root, path.relative(inventory.root, file)) : file;
        projectPaths.set(project, editorPath);
        watchedRoots.push(...(inventory.source_roots || []).map(editorPath));
        if (inventory.output_root) outputRoots.push(editorPath(inventory.output_root));
        const files = [];
        for (const entry of inventory.files) {
          const uri = vscode.Uri.file(editorPath(entry.path));
          const file = project.children.get(uri.toString()) || controller.createTestItem(uri.toString(), entry.selector, uri);
          file.error = entry.error || undefined;
          data.set(file, { root, project, selector: entry.selector });
          const previous = new Map(descendants(file).map(item => [item.id, item]));
          const suites = new Map(), nextChildren = new Map([[file, []]]);
          for (const suite of entry.suites || []) {
            if (suites.has(suite.full_name)) continue;
            const id = `${file.id}::suite::${suite.full_name}`;
            const item = previous.get(id) || controller.createTestItem(id, suite.name, uri);
            item.range = sourceRange(suite.line, suite.column);
            data.set(item, { ...data.get(file), suiteName: suite.full_name });
            const parent = suites.get(suite.parent) || file;
            nextChildren.get(parent).push(item);
            nextChildren.set(item, []);
            suites.set(suite.full_name, item);
          }
          for (const test of entry.cases) {
            const parent = suites.get(test.suite) || file;
            const item = caseItem(file, test.full_name, test.line, test.column, false, parent);
            item.label = test.name || test.full_name;
            nextChildren.get(parent).push(item);
          }
          for (const [parent, items] of nextChildren) parent.children.replace([...new Set(items)]);
          caseItems.set(file, new Map(descendants(file).filter(item => data.get(item)?.caseName !== undefined).map(item => [item.id, item])));
          files.push(file);
        }
        project.error = undefined;
        project.children.replace(files);
      } catch (error) { project.error = error.message; }
    }
    if (!token?.isCancellationRequested) controller.items.replace(roots().filter(item => active.has(item.id)));
  }

  async function run(request, token, withCoverage = false) {
    if (!vscode.workspace.isTrusted) {
      await vscode.window.showWarningMessage("Trust this workspace before running Tua tests.");
      return;
    }
    if (!initialized) await refresh(token);
    const run = controller.createTestRun(request);
    const coverageFiles = new Map();
    const jobs = planRuns(roots(), request, data);
    try {
      for (const job of jobs) {
        if (token?.isCancellationRequested) break;
        const expected = [job.item, ...descendants(job.item).filter(item => data.get(item)?.caseName !== undefined)];
        expected.forEach(item => run.enqueued(item));
        run.started(job.item);
        const temporary = await fs.mkdtemp(path.join(os.tmpdir(), "tua-test-explorer-"));
        try {
          const reportPath = path.join(temporary, "results.json");
          const settings = vscode.workspace.getConfiguration("tua", job.item.uri);
          const coveragePath = withCoverage ? path.join(temporary, "coverage.json") : undefined;
          const result = await invoke(testArguments(job, reportPath, settings, coveragePath), { cwd: job.root, token, truncateOutput: true });
          run.appendOutput((result.stdout + result.stderr).replace(/\r?\n/g, "\r\n"));
          if (result.cancelled) { expected.forEach(item => run.skipped(item)); break; }
          let report;
          try { report = JSON.parse(await fs.readFile(reportPath, "utf8")); }
          catch (error) { throw new Error(result.stderr || result.stdout || error.message); }
          if (!report.completed || report.version !== 1 || !Array.isArray(report.tests)) throw new Error(result.stderr || "Test run did not complete");
          if (coveragePath) mergeCoverage(coverageFiles, JSON.parse(await fs.readFile(coveragePath, "utf8")));
          const observed = new Set();
          const failed = new Set();
          for (const result of report.tests) {
            const resolvedPath = result.file ? path.resolve(job.root, result.file) : undefined;
            const filePath = resolvedPath && (projectPaths.get(job.project)?.(resolvedPath) || resolvedPath);
            let file = children(job.project).find(file => file.uri?.fsPath === filePath);
            if (!file) {
              const uri = vscode.Uri.file(filePath || path.join(job.root, "tua.toml"));
              file = controller.createTestItem(uri.toString(), path.basename(uri.fsPath), uri);
              data.set(file, { root: job.root, project: job.project, expandOnly: true });
              job.project.children.add(file);
            }
            const item = caseItem(file, result.name, result.line, 1, true);
            observed.add(item.id);
            if (result.status === "passed") { if (!failed.has(item.id)) run.passed(item); }
            else if (result.status === "skipped") { if (!failed.has(item.id)) run.skipped(item); }
            else {
              failed.add(item.id);
              const message = new vscode.TestMessage(result.message || result.status);
              if (filePath) message.location = new vscode.Location(vscode.Uri.file(filePath), sourceRange(result.line));
              if (result.status === "failed") run.failed(item, message); else run.errored(item, message);
            }
          }
          for (const item of expected) {
            if (data.get(item)?.caseName !== undefined && !observed.has(item.id)) run.skipped(item);
          }
          if (result.code !== 0 && failed.size === 0) run.errored(job.item, new vscode.TestMessage(result.stderr || "Busted exited unsuccessfully"));
          else if (failed.size) run.failed(job.item, new vscode.TestMessage(`${failed.size} test(s) failed`));
          else if (!report.tests.length || report.tests.every(test => test.status === "skipped")) run.skipped(job.item);
          else run.passed(job.item);
        } catch (error) { run.errored(job.item, new vscode.TestMessage(error.message)); }
        finally { await fs.rm(temporary, { recursive: true, force: true }); }
      }
    } finally {
      try { if (withCoverage) publishCoverage(vscode, run, coverageFiles, coverageDetails); }
      finally { run.end(); }
    }
  }

  const schedule = (allowInitialize = false) => {
    if ((!initialized && !allowInitialize) || disposed) return;
    clearTimeout(timer);
    timer = setTimeout(() => {
      if (refreshQueued) return;
      refreshQueued = true;
      enqueue(async () => {
        refreshQueued = false;
        await refresh();
        for (const session of continuous) {
          if (!session.token.isCancellationRequested) {
            // Rebind selections to refreshed items by stable IDs.
            const all = roots().flatMap(root => [root, ...descendants(root)]);
            const select = items => items?.map(item => all.find(next => next.id === item.id)).filter(Boolean);
            await run(new vscode.TestRunRequest(select(session.request.include), select(session.request.exclude), profile, false), session.token);
          }
        }
      }).catch(error => vscode.window.showErrorMessage(error.message));
    }, 250);
  };
  const profile = controller.createRunProfile("Run with Busted", vscode.TestRunProfileKind.Run, (request, token) => {
    if (request.continuous && !token.isCancellationRequested) {
      const session = { request, token };
      continuous.add(session);
      session.subscription = token.onCancellationRequested(() => { continuous.delete(session); session.subscription.dispose(); });
    }
    return enqueue(() => run(request, token));
  }, true);
  profile.supportsContinuousRun = true;
  const coverageProfile = controller.createRunProfile("Coverage with LuaCov", vscode.TestRunProfileKind.Coverage,
    (request, token) => enqueue(() => run(request, token, true)), true);
  coverageProfile.loadDetailedCoverage = (_run, coverage) => coverageDetails.get(coverage) || [];
  controller.resolveHandler = () => enqueue(() => refresh());
  controller.refreshHandler = token => enqueue(() => refresh(token));
  // Watch mode is explicit through the Testing UI; edits alone never run tests.
  for (const folder of vscode.workspace.workspaceFolders || []) {
    const watcher = vscode.workspace.createFileSystemWatcher(new vscode.RelativePattern(folder, "**/*"));
    const changed = uri => {
      const file = uri.fsPath;
      if (file.split(path.sep).some(part => [".tua", "node_modules", ".git"].includes(part)) || outputRoots.some(root => within(file, root))) return;
      if (configPaths.includes(file) || path.basename(file) === "tua.toml" || watchedRoots.some(root => within(file, root))) schedule();
    };
    watchers.push(watcher, watcher.onDidCreate(changed), watcher.onDidChange(changed), watcher.onDidDelete(changed));
  }
  // Opening a source file should reveal native Testing gutter actions without
  // first opening the Testing view. Discovery never executes spec code.
  const discoverForDocument = document => {
    if (initialized || !vscode.workspace.isTrusted || document?.uri?.scheme !== "file") return;
    if (!["tua", "lua"].includes(document.languageId) && !/\.(tua|lua)$/.test(document.uri.fsPath)) return;
    if (!(vscode.workspace.workspaceFolders || []).some(folder => within(document.uri.fsPath, folder.uri.fsPath))) return;
    schedule(true);
  };
  const editorListeners = [
    vscode.workspace.onDidOpenTextDocument?.(discoverForDocument),
    vscode.window.onDidChangeActiveTextEditor?.(editor => discoverForDocument(editor?.document)),
    vscode.workspace.onDidGrantWorkspaceTrust?.(() => discoverForDocument(vscode.window.activeTextEditor?.document)),
  ].filter(Boolean);
  discoverForDocument(vscode.window.activeTextEditor?.document);
  return vscode.Disposable.from(controller, profile, coverageProfile, ...watchers, ...editorListeners,
    vscode.commands.registerCommand("tua.tests.refresh", () => enqueue(() => refresh())),
    { dispose() { disposed = true; clearTimeout(timer); for (const session of continuous) session.subscription.dispose(); continuous.clear(); [...cancellations].forEach(cancel => cancel()); cancellations.clear(); } });
}

module.exports = { registerTesting };
