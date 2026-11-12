"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn } = require("node:child_process");

const ALGORITHMS = new Set(["sidewinder", "binaryTree", "arena"]);
const DEFAULTS = Object.freeze({ width: 31, height: 21, seed: 817, algorithm: "sidewinder", radius: 8 });
const START = "TUA_ROT_PREVIEW_V1";
const END = "TUA_ROT_PREVIEW_END";

function validateOptions(value) {
  const options = { ...DEFAULTS, ...value };
  for (const key of ["width", "height"]) {
    if (!Number.isInteger(options[key]) || options[key] < 11 || options[key] > 41 || options[key] % 2 === 0) {
      throw new Error(`${key} must be an odd integer from 11 to 41.`);
    }
  }
  if (!Number.isSafeInteger(options.seed) || options.seed < 1 || options.seed > 1000000000) {
    throw new Error("Seed must be an integer from 1 to 1,000,000,000.");
  }
  if (!ALGORITHMS.has(options.algorithm)) throw new Error("Unknown map algorithm.");
  if (!Number.isInteger(options.radius) || options.radius < 1 || options.radius > 20) {
    throw new Error("Visibility radius must be an integer from 1 to 20.");
  }
  return options;
}

function previewSource(options) {
  const { width, height, seed, algorithm, radius } = validateOptions(options);
  return `function love.load()
  local rng = Rot.Rng.new(${seed})
  local maze = Rot.Map.generate(${width}, ${height}, "${algorithm}", rng)
  local route = Rot.Path.astar(maze, 2, 2, ${width - 1}, ${height - 1})
  local visible = Rot.FOV.compute(${width}, ${height}, 2, 2, ${radius}, function(x, y)
    return not maze:get(x, y)
  end)
  print("${START}")
  for y = 1, ${height} do
    local walls = ""
    local sight = ""
    for x = 1, ${width} do
      walls = walls .. (maze:get(x, y) and "1" or "0")
      sight = sight .. (visible:get(x, y) and "1" or "0")
    end
    print("M:" .. walls)
    print("V:" .. sight)
  end
  local points = {}
  if route then
    for _, point in ipairs(route) do
      points[#points + 1] = point.x .. "," .. point.y
    end
  end
  print("P:" .. table.concat(points, ";"))
  print("${END}")
  love.event.quit(0)
end
`;
}

function parsePreview(output, options) {
  const { width, height } = validateOptions(options);
  const lines = output.split(/\r?\n/u);
  const start = lines.lastIndexOf(START);
  if (start < 0 || lines[start + 2 * height + 2] !== END) {
    throw new Error("LÖVE did not return a complete procedural preview.");
  }
  const map = [], visible = [];
  for (let y = 0; y < height; y++) {
    const mapLine = lines[start + 1 + 2 * y];
    const visibleLine = lines[start + 2 + 2 * y];
    if (!mapLine?.startsWith("M:") || !visibleLine?.startsWith("V:") ||
        !new RegExp(`^[01]{${width}}$`, "u").test(mapLine.slice(2)) ||
        !new RegExp(`^[01]{${width}}$`, "u").test(visibleLine.slice(2))) {
      throw new Error(`Invalid procedural preview row ${y + 1}.`);
    }
    map.push(mapLine.slice(2));
    visible.push(visibleLine.slice(2));
  }
  const pathLine = lines[start + 1 + 2 * height];
  if (!pathLine?.startsWith("P:")) throw new Error("Missing procedural preview path.");
  const points = pathLine.slice(2) ? pathLine.slice(2).split(";").map((entry) => {
    if (!/^\d+,\d+$/u.test(entry)) throw new Error("Invalid procedural preview path point.");
    const [x, y] = entry.split(",").map(Number);
    if (x < 1 || y < 1 || x > width || y > height || map[y - 1][x - 1] !== "1") {
      throw new Error("Path point lies outside the walkable map.");
    }
    return { x, y };
  }) : [];
  return { ...validateOptions(options), map, visible, path: points };
}

function run(command, args, cwd, onChild, timeoutMs) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd, stdio: ["ignore", "pipe", "pipe"] });
    onChild(child);
    let stdout = "", stderr = "", settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) reject(error); else resolve(stdout);
    };
    const timer = setTimeout(() => { child.kill(); finish(new Error(`${path.basename(command)} exceeded ${timeoutMs / 1000} seconds.`)); }, timeoutMs);
    child.stdout.on("data", (data) => { stdout += data; if (stdout.length > 131072) { child.kill(); finish(new Error("Procedural preview output exceeded its limit.")); } });
    child.stderr.on("data", (data) => { stderr += data; if (stderr.length > 32768) stderr = stderr.slice(-32768); });
    child.once("error", finish);
    child.once("exit", (code) => finish(code === 0 ? null : new Error(stderr.trim() || stdout.trim() || `${path.basename(command)} exited ${code}.`)));
  });
}

function createProceduralRunner({ getServerCommand, getLoveCommand }) {
  let active, generation = 0;
  return {
    stop() { generation++; active?.kill(); active = undefined; },
    async generate(input) {
      const options = validateOptions(input);
      this.stop();
      const current = generation;
      const temp = fs.mkdtempSync(path.join(os.tmpdir(), "tua-rot-preview-"));
      try {
        fs.mkdirSync(path.join(temp, "src"));
        fs.writeFileSync(path.join(temp, "tua.toml"), 'source = "src"\nout = "build"\nmain = "main.tua"\ntarget = "love11-luajit"\n');
        fs.writeFileSync(path.join(temp, "src/main.tua"), previewSource(options));
        await run(getServerCommand(), ["build"], temp, (child) => { if (current === generation) active = child; else child.kill(); }, 20000);
        if (current !== generation) throw new Error("Preview cancelled.");
        fs.writeFileSync(path.join(temp, "build/conf.lua"), "function love.conf(t) t.window.visible=false; t.window.width=64; t.window.height=64 end\n");
        const output = await run(getLoveCommand(), [path.join(temp, "build")], temp, (child) => { if (current === generation) active = child; else child.kill(); }, 10000);
        if (current !== generation) throw new Error("Preview cancelled.");
        return parsePreview(output, options);
      } finally {
        if (current === generation) active = undefined;
        fs.rmSync(temp, { recursive: true, force: true });
      }
    },
  };
}

module.exports = { DEFAULTS, createProceduralRunner, parsePreview, previewSource, validateOptions };
