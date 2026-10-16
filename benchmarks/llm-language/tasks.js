"use strict";

const CURSOR = "<TUA_COMPLETION>";

function task(id, category, card, instruction, prefix, answer, suffix = "", options = {}) {
  return {
    id,
    category,
    card,
    instruction,
    files: options.files || {},
    source: `${prefix}${CURSOR}${suffix}`,
    answer,
    mutant: options.mutant || `${answer}\nlocal __benchmark_type_error: number = \"wrong\"\n`,
    mustMatch: options.mustMatch || [],
    mustNotMatch: options.mustNotMatch || ["```", "<\\|fim_"],
  };
}

const tasks = [
  task("lua-local-type", "lua", "core", "Declare score as a number with value 10.", "", "local score: number = 10\n", "print(score)\n", { mustMatch: ["^local score: number = 10"] }),
  task("lua-record-value", "lua", "types", "Create a typed point value.", "type Point = { x: number, y: number }\n", "local point: Point = { x = 2, y = 4 }\n", "local x: number = point.x\n", { mustMatch: ["point: Point"] }),
  task("lua-if-narrow", "lua", "types", "Print the string only after narrowing it.", "local value: string | nil = \"ready\"\n", "if value ~= nil then\n  print(value)\nend\n", "", { mustMatch: ["if value ~= nil then"] }),
  task("lua-numeric-for", "lua", "core", "Sum integers one through five using Lua's numeric for loop.", "local total: number = 0\n", "for i = 1, 5 do\n  total += i\nend\n", "local result: number = total\n", { mustMatch: ["for i = 1, 5 do", "total \\+= i"] }),
  task("lua-ipairs", "lua", "types", "Print every name with ipairs.", "local names: string[] = { \"Ada\", \"Lin\" }\n", "for _, name in ipairs(names) do\n  print(name)\nend\n", "", { mustMatch: ["ipairs\\(names\\)"] }),
  task("lua-anonymous-function", "lua", "functions", "Assign a typed anonymous function that doubles a number.", "", "local double = function(value: number): number\n  return value * 2\nend\n", "local result: number = double(4)\n", { mustMatch: ["function\\(value: number\\): number", "return value \\* 2"] }),

  task("type-record", "types", "types", "Define the Player record type used below.", "", "type Player = { name: string, hp: number }\n", "local player: Player = { name = \"Mina\", hp = 100 }\n", { mustMatch: ["type Player ="] }),
  task("type-array", "types", "types", "Declare an array of numbers.", "", "local scores: number[] = { 10, 20, 30 }\n", "local first: number = scores[1]\n", { mustMatch: ["number\\[\\]"] }),
  task("type-union", "types", "types", "Define a string-or-number union and use it.", "", "type Identifier = string | number\nlocal id: Identifier = \"player\"\n", "print(id)\n", { mustMatch: ["string \\| number"] }),
  task("type-optional-field", "types", "types", "Define SaveData with an optional checkpoint field.", "", "type SaveData = { name: string, checkpoint?: string }\n", "local save: SaveData = { name = \"slot-1\" }\n", { mustMatch: ["checkpoint\\?: string"] }),
  task("type-literal-union", "types", "types", "Define the two allowed loading modes.", "", "type LoadMode = \"static\" | \"stream\"\n", "local mode: LoadMode = \"stream\"\n", { mustMatch: ["\\\"static\\\" \\| \\\"stream\\\""] }),
  task("type-any-boundary", "types", "types", "Annotate the legacy result as dynamic any.", "local function legacy()\n  return { deep = { value = 3 } }\nend\n", "local payload: any = legacy()\n", "local value = payload.deep.value\n", { mustMatch: ["payload: any"] }),

  task("function-typed", "functions", "functions", "Write a typed add function.", "", "local function add(a: number, b: number): number\n  return a + b\nend\n", "local sum: number = add(2, 3)\n", { mustMatch: ["local function add\\(a: number, b: number\\): number"] }),
  task("function-default", "functions", "functions", "Write heal with a default amount of 5.", "", "local function heal(hp: number, amount: number = 5): number\n  return hp + amount\nend\n", "local nextHp: number = heal(10)\n", { mustMatch: ["amount: number = 5"] }),
  task("function-optional", "functions", "functions", "Write a function with an optional trailing label.", "", "local function describe(value: number, label?: string): string\n  if label ~= nil then\n    return label .. tostring(value)\n  end\n  return tostring(value)\nend\n", "local text: string = describe(3)\n", { mustMatch: ["label\\?: string"], mutant: "local function describe(value: number, label: string): string\n  return label .. tostring(value)\nend\n" }),
  task("function-multi-return", "functions", "functions", "Return width and height as two inferred Lua return values.", "", "local function dimensions()\n  return 320, 180\nend\n", "local width, height = dimensions()\nlocal checkedWidth: number = width\nlocal checkedHeight: number = height\n", { mustMatch: ["return 320, 180"] }),
  task("function-all-paths", "functions", "functions", "Return a string on every branch.", "", "local function status(alive: boolean): string\n  if alive then\n    return \"alive\"\n  end\n  return \"down\"\nend\n", "local text: string = status(true)\n", { mustMatch: ["return \\\"alive\\\"", "return \\\"down\\\""] }),
  task("function-recursive", "functions", "functions", "Write a typed recursive factorial function.", "", "local function factorial(value: number): number\n  if value <= 1 then\n    return 1\n  end\n  return value * factorial(value - 1)\nend\n", "local result: number = factorial(5)\n", { mustMatch: ["factorial\\(value - 1\\)"] }),

  task("module-require", "modules", "modules", "Require the runtime module and call its exported function.", "", "local Math = require(\"math_utils\")\nlocal total: number = Math.add(2, 4)\n", "print(total)\n", { files: { "math_utils.tua": "local function add(a: number, b: number): number\n  return a + b\nend\nreturn { add = add }\n" }, mustMatch: ["require\\(\\\"math_utils\\\"\\)"] }),
  task("module-nested-require", "modules", "modules", "Load the nested player module by its project module name.", "", "local Player = require(\"entities/player\")\nlocal name: string = Player.name\n", "print(name)\n", { files: { "entities/player.tua": "return { name = \"Mina\" }\n" }, mustMatch: ["require\\(\\\"entities/player\\\"\\)"] }),
  task("module-type-export", "modules", "modules", "Export the Player type for qualified use by another Tua module.", "", "export type Player = { name: string, hp: number }\n", "local sample: Player = { name = \"Ada\", hp = 10 }\nreturn { sample = sample }\n", { mustMatch: ["export type Player"] }),
  task("module-chain", "modules", "modules", "Require the facade and use the definition exported through two layers.", "", "local Facade = require(\"facade\")\nlocal score: number = Facade.score\n", "print(score)\n", { files: { "base.tua": "return { score = 42 }\n", "facade.tua": "local Base = require(\"base\")\nreturn { score = Base.score }\n" }, mustMatch: ["Facade.score"] }),

  task("class-basic", "classes", "classes", "Define a class with a typed field and initializer.", "", "class Counter {\n  value: number\n\n  init(value: number) {\n    self.value = value\n  }\n}\n", "local counter = Counter.new(1)\nlocal value: number = counter.value\n", { mustMatch: ["class Counter", "init\\(value: number\\)"] }),
  task("class-method", "classes", "classes", "Add a typed method returning self.", "class Counter {\n  value: number = 0\n\n", "  increment(amount: number): Self {\n    self.value += amount\n    return self\n  }\n", "}\nlocal counter = Counter.new()\ncounter:increment(2)\n", { mustMatch: ["increment\\(amount: number\\): Self"] }),
  task("class-inheritance", "classes", "classes", "Define Player as a final subclass of the open Entity class.", "open class Entity {\n  x: number = 0\n}\n", "final class Player extends Entity {\n  hp: number = 100\n}\n", "local player = Player.new()\nlocal x: number = player.x\n", { mustMatch: ["final class Player extends Entity"] }),
  task("class-super", "classes", "classes", "Initialize the parent fields with super, then initialize hp.", "open class Entity {\n  x: number\n  init(x: number) { self.x = x }\n}\nfinal class Player extends Entity {\n  hp: number\n", "  init(x: number, hp: number) {\n    super(x)\n    self.hp = hp\n  }\n", "}\nlocal player = Player.new(2, 10)\n", { mustMatch: ["super\\(x\\)"] }),
  task("class-override", "classes", "classes", "Override update and mark the override final.", "open class Entity {\n  update(dt: number) {}\n}\nfinal class Player extends Entity {\n", "  override final update(dt: number) {\n    print(dt)\n  }\n", "}\nlocal player = Player.new()\nplayer:update(0.1)\n", { mustMatch: ["override final update"] }),

  task("love-load", "love", "love", "Define LOVE's load callback and initialize score.", "local score: number = 0\n", "function love.load()\n  score = 10\nend\n", "", { mustMatch: ["function love.load\\(\\)"] }),
  task("love-update", "love", "love", "Define LOVE's typed update callback.", "local elapsed: number = 0\n", "function love.update(dt: number)\n  elapsed += dt\nend\n", "", { mustMatch: ["love.update\\(dt: number\\)"] }),
  task("love-draw", "love", "love", "Draw text with the existing LOVE API.", "local message: string = \"hello\"\n", "function love.draw()\n  love.graphics.print(message, 10, 20)\nend\n", "", { mustMatch: ["love.graphics.print"] }),
  task("love-colon-method", "love", "love", "Call the Source instance method with colon syntax.", "local source = love.audio.newQueueableSource(44100, 16, 1, 2)\n", "source:play()\n", "", { mustMatch: ["source:play\\(\\)"], mustNotMatch: ["```", "<\\|fim_", "source\\.play\\("] }),
  task("love-vector-helper", "love", "love", "Create a Vec2 with Tua's vec2 helper.", "", "local position: Vec2 = vec2(10, 20)\n", "local x: number = position.x\n", { mustMatch: ["vec2\\(10, 20\\)"] }),

  task("compound-local", "operators", "core", "Increase score with compound assignment.", "local score: number = 7\n", "score += 3\n", "local result: number = score\n", { mustMatch: ["score \\+= 3"] }),
  task("compound-field", "operators", "core", "Subtract damage from the nested hp field with compound assignment.", "local state = { player = { hp = 20 } }\n", "state.player.hp -= 4\n", "local hp: number = state.player.hp\n", { mustMatch: ["state.player.hp -= 4"] }),
  task("compound-deep-field", "operators", "core", "Double a nested speed field with compound assignment.", "local state = { player = { speed = 3 } }\n", "state.player.speed *= 2\n", "local speed: number = state.player.speed\n", { mustMatch: ["state.player.speed \\*= 2"] }),
  task("deep-field", "operators", "types", "Read the deeply nested numeric definition without replacing it with any.", "local registry = { game = { actors = { player = { stats = { hp = 100 } } } } }\n", "local hp: number = registry.game.actors.player.stats.hp\n", "print(hp)\n", { mustMatch: ["registry.game.actors.player.stats.hp"], mustNotMatch: ["```", "<\\|fim_", ": any"] }),

  task("trap-no-generics", "negative", "core", "Write an identity function using supported Tua syntax; do not invent generics.", "", "local function identity(value: any): any\n  return value\nend\n", "local result = identity(3)\n", { mustNotMatch: ["```", "<\\|fim_", "<T>", "<T extends"] }),
  task("trap-no-typescript", "negative", "core", "Declare a typed options table using Tua syntax, not TypeScript.", "", "type Options = { volume: number }\nlocal options: Options = { volume = 0.5 }\n", "print(options.volume)\n", { mustNotMatch: ["```", "<\\|fim_", "interface ", "const ", "let "] }),
  task("trap-no-luau-cast", "negative", "core", "Use a normal Tua annotation, not Luau's cast operator.", "", "local value: number = 3\n", "print(value)\n", { mustNotMatch: ["```", "<\\|fim_", "::"] }),
  task("trap-no-invented-love", "negative", "love", "Use LOVE's real rectangle call; do not invent a drawRectangle API.", "function love.draw()\n", "  love.graphics.rectangle(\"fill\", 10, 20, 30, 40)\n", "end\n", { mustMatch: ["love.graphics.rectangle"], mustNotMatch: ["```", "<\\|fim_", "drawRectangle"] }),
];

module.exports = { CURSOR, tasks };
