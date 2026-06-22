use crate::love_api;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LuaItem {
    pub label: &'static str,
    pub kind: LuaItemKind,
    pub detail: &'static str,
    pub documentation: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LuaItemKind {
    Function,
    Keyword,
    Module,
    Value,
}

pub(crate) fn keyword_items(prefix: &str) -> Vec<LuaItem> {
    KEYWORDS
        .iter()
        .copied()
        .filter(|item| item.label.starts_with(prefix))
        .collect()
}

pub(crate) fn top_level_items(prefix: &str) -> Vec<LuaItem> {
    TOP_LEVEL
        .iter()
        .copied()
        .filter(|item| item.label.starts_with(prefix))
        .collect()
}

pub(crate) fn field_items(receiver_parts: &[String], prefix: &str) -> Vec<LuaItem> {
    if receiver_is_love(receiver_parts) {
        let love_items = love_api::field_items(receiver_parts, prefix);

        if !love_items.is_empty() {
            return love_items;
        }
    }

    let Some(items) = items_for_receiver(receiver_parts) else {
        return Vec::new();
    };

    items
        .iter()
        .copied()
        .filter(|item| item.label.starts_with(prefix))
        .collect()
}

pub(crate) fn hover_item(parts: &[String]) -> Option<LuaItem> {
    let (name, receiver) = parts.split_last()?;

    if receiver.is_empty() {
        return TOP_LEVEL.iter().find(|item| item.label == name).copied();
    }

    if receiver_is_love(parts) {
        if let Some(item) = love_api::hover_item(parts) {
            return Some(item);
        }
    }

    items_for_receiver(receiver)
        .and_then(|items| items.iter().find(|item| item.label == name).copied())
}

fn receiver_is_love(parts: &[String]) -> bool {
    parts.first().is_some_and(|part| part == "love")
}

fn items_for_receiver(receiver_parts: &[String]) -> Option<&'static [LuaItem]> {
    match receiver_parts {
        [root] if root == "coroutine" => Some(COROUTINE_ITEMS),
        [root] if root == "debug" => Some(DEBUG_ITEMS),
        [root] if root == "io" => Some(IO_ITEMS),
        [root] if root == "love" => Some(LOVE_ITEMS),
        [root] if root == "math" => Some(MATH_ITEMS),
        [root] if root == "os" => Some(OS_ITEMS),
        [root] if root == "package" => Some(PACKAGE_ITEMS),
        [root] if root == "string" => Some(STRING_ITEMS),
        [root] if root == "table" => Some(TABLE_ITEMS),
        [root, child] if root == "love" && child == "audio" => Some(LOVE_AUDIO_ITEMS),
        [root, child] if root == "love" && child == "event" => Some(LOVE_EVENT_ITEMS),
        [root, child] if root == "love" && child == "filesystem" => Some(LOVE_FILESYSTEM_ITEMS),
        [root, child] if root == "love" && child == "graphics" => Some(LOVE_GRAPHICS_ITEMS),
        [root, child] if root == "love" && child == "keyboard" => Some(LOVE_KEYBOARD_ITEMS),
        [root, child] if root == "love" && child == "mouse" => Some(LOVE_MOUSE_ITEMS),
        [root, child] if root == "love" && child == "timer" => Some(LOVE_TIMER_ITEMS),
        [root, child] if root == "love" && child == "window" => Some(LOVE_WINDOW_ITEMS),
        _ => None,
    }
}

const fn item(
    label: &'static str,
    kind: LuaItemKind,
    detail: &'static str,
    documentation: &'static str,
) -> LuaItem {
    LuaItem {
        label,
        kind,
        detail,
        documentation,
    }
}

const KEYWORDS: &[LuaItem] = &[
    item("and", LuaItemKind::Keyword, "Lua keyword", "logical and"),
    item(
        "break",
        LuaItemKind::Keyword,
        "Lua keyword",
        "leave the current loop",
    ),
    item("do", LuaItemKind::Keyword, "Lua keyword", "start a block"),
    item(
        "else",
        LuaItemKind::Keyword,
        "Lua keyword",
        "alternate branch",
    ),
    item(
        "elseif",
        LuaItemKind::Keyword,
        "Lua keyword",
        "alternate conditional branch",
    ),
    item("end", LuaItemKind::Keyword, "Lua keyword", "close a block"),
    item(
        "false",
        LuaItemKind::Keyword,
        "Lua keyword",
        "boolean false",
    ),
    item(
        "for",
        LuaItemKind::Keyword,
        "Lua keyword",
        "start a for loop",
    ),
    item(
        "function",
        LuaItemKind::Keyword,
        "Lua keyword",
        "declare a function",
    ),
    item(
        "if",
        LuaItemKind::Keyword,
        "Lua keyword",
        "start a conditional",
    ),
    item(
        "in",
        LuaItemKind::Keyword,
        "Lua keyword",
        "iterator separator",
    ),
    item(
        "local",
        LuaItemKind::Keyword,
        "Lua keyword",
        "declare a local binding",
    ),
    item(
        "nil",
        LuaItemKind::Keyword,
        "Lua keyword",
        "absence of a value",
    ),
    item(
        "not",
        LuaItemKind::Keyword,
        "Lua keyword",
        "logical negation",
    ),
    item("or", LuaItemKind::Keyword, "Lua keyword", "logical or"),
    item(
        "repeat",
        LuaItemKind::Keyword,
        "Lua keyword",
        "start a repeat loop",
    ),
    item(
        "return",
        LuaItemKind::Keyword,
        "Lua keyword",
        "return from a function or chunk",
    ),
    item(
        "then",
        LuaItemKind::Keyword,
        "Lua keyword",
        "start an if branch body",
    ),
    item("true", LuaItemKind::Keyword, "Lua keyword", "boolean true"),
    item(
        "until",
        LuaItemKind::Keyword,
        "Lua keyword",
        "close a repeat loop",
    ),
    item(
        "while",
        LuaItemKind::Keyword,
        "Lua keyword",
        "start a while loop",
    ),
];

const TOP_LEVEL: &[LuaItem] = &[
    item(
        "_ENV",
        LuaItemKind::Value,
        "global environment table",
        "The current global environment table for a chunk or function.",
    ),
    item(
        "_G",
        LuaItemKind::Value,
        "global environment table",
        "A global variable that holds the global environment. Lua itself does not use this variable; changing it does not change any environment.",
    ),
    item(
        "_VERSION",
        LuaItemKind::Value,
        "Lua version string",
        "The Lua runtime version string.",
    ),
    item(
        "arg",
        LuaItemKind::Value,
        "table",
        "Command-line argument table provided by standalone Lua hosts.",
    ),
    item(
        "assert",
        LuaItemKind::Function,
        "function assert(v: any, message?: string): any",
        "Raises an error if v is false or nil; otherwise returns its arguments.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-assert",
    ),
    item(
        "collectgarbage",
        LuaItemKind::Function,
        "function collectgarbage(opt?, arg?: number): any",
        "Generic interface to Lua's garbage collector. Common options include \"collect\", \"stop\", \"restart\", \"count\", \"step\", \"setpause\", \"setstepmul\", \"incremental\", \"generational\", and \"isrunning\".\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-collectgarbage",
    ),
    item(
        "coroutine",
        LuaItemKind::Module,
        "Lua coroutine library",
        "Cooperative threading primitives.",
    ),
    item(
        "debug",
        LuaItemKind::Module,
        "Lua debug library",
        "Debug and introspection helpers.",
    ),
    item(
        "dofile",
        LuaItemKind::Function,
        "function dofile(filename?: string): any",
        "Opens the named file and executes its contents as a Lua chunk. Without a filename, executes standard input.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-dofile",
    ),
    item(
        "error",
        LuaItemKind::Function,
        "function error(message: string, level?: number)",
        "Terminates the last protected function called and returns message as the error object.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-error",
    ),
    item(
        "getfenv",
        LuaItemKind::Function,
        "function getfenv(f?): table",
        "Gets a function environment in Lua 5.1.",
    ),
    item(
        "getmetatable",
        LuaItemKind::Function,
        "function getmetatable(object: any): table?",
        "Returns an object's metatable, nil when it has none, or the protected __metatable value.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-getmetatable",
    ),
    item(
        "io",
        LuaItemKind::Module,
        "Lua io library",
        "File and stream I/O helpers.",
    ),
    item(
        "ipairs",
        LuaItemKind::Function,
        "function ipairs(t: table)",
        "Returns an iterator over array-like table entries from index 1 up to the first absent index.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-ipairs",
    ),
    item(
        "load",
        LuaItemKind::Function,
        "function load(chunk, chunkname?: string, mode?, env?): function?",
        "Loads a chunk from a string or reader function. Returns the compiled chunk, or nil plus an error message.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-load",
    ),
    item(
        "loadfile",
        LuaItemKind::Function,
        "function loadfile(filename?: string, mode?, env?): function?",
        "Loads a chunk from a file or standard input without running it.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-loadfile",
    ),
    item(
        "loadstring",
        LuaItemKind::Function,
        "function loadstring(text: string, chunkname?: string): function",
        "Loads Lua source as a function in Lua 5.1/LuaJIT.",
    ),
    item(
        "love",
        LuaItemKind::Module,
        "LÖVE framework module",
        "The LÖVE runtime API root.",
    ),
    item(
        "math",
        LuaItemKind::Module,
        "Lua math library",
        "Mathematical constants and functions.",
    ),
    item(
        "module",
        LuaItemKind::Function,
        "function module(name, ...)",
        "Lua 5.1 module helper.",
    ),
    item(
        "next",
        LuaItemKind::Function,
        "function next(table: table, index?: any): any",
        "Returns the next key and value in a table. Use next(t) to check whether a table is empty.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-next",
    ),
    item(
        "os",
        LuaItemKind::Module,
        "Lua os library",
        "Operating-system helpers exposed by Lua.",
    ),
    item(
        "package",
        LuaItemKind::Module,
        "Lua package library",
        "Module loading state and search paths.",
    ),
    item(
        "pairs",
        LuaItemKind::Function,
        "function pairs(t: table)",
        "Returns an iterator over all key/value pairs in a table.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-pairs",
    ),
    item(
        "pcall",
        LuaItemKind::Function,
        "function pcall(f: function, ...): boolean",
        "Calls a function in protected mode and returns whether it succeeded, followed by return values or an error object.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-pcall",
    ),
    item(
        "print",
        LuaItemKind::Function,
        "function print(...)",
        "Receives any number of arguments and prints their string representations to stdout.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-print",
    ),
    item(
        "rawequal",
        LuaItemKind::Function,
        "function rawequal(v1: any, v2: any): boolean",
        "Checks whether two values are equal without invoking the __eq metamethod.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-rawequal",
    ),
    item(
        "rawget",
        LuaItemKind::Function,
        "function rawget(table: table, index: any): any",
        "Gets the real value of table[index] without invoking the __index metamethod.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-rawget",
    ),
    item(
        "rawlen",
        LuaItemKind::Function,
        "function rawlen(v: string | table): number",
        "Returns the length of a table or string without invoking metamethods.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-rawlen",
    ),
    item(
        "rawset",
        LuaItemKind::Function,
        "function rawset(table: table, index: any, value: any): table",
        "Sets table[index] to value without invoking the __newindex metamethod.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-rawset",
    ),
    item(
        "require",
        LuaItemKind::Function,
        "function require(modname: string): any",
        "Loads a Lua module by name, using package.loaded, package.preload, package.path, and package.cpath.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-require",
    ),
    item(
        "select",
        LuaItemKind::Function,
        "function select(index: number | string, ...): any",
        "Returns selected values from varargs, or the count when index is \"#\".\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-select",
    ),
    item(
        "setfenv",
        LuaItemKind::Function,
        "function setfenv(target, env)",
        "Sets a function environment in Lua 5.1.",
    ),
    item(
        "setmetatable",
        LuaItemKind::Function,
        "function setmetatable(table: table, metatable?: table): table",
        "Sets or clears a table metatable, respecting protected __metatable fields.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-setmetatable",
    ),
    item(
        "string",
        LuaItemKind::Module,
        "Lua string library",
        "String processing functions.",
    ),
    item(
        "table",
        LuaItemKind::Module,
        "Lua table library",
        "Table helpers.",
    ),
    item(
        "tonumber",
        LuaItemKind::Function,
        "function tonumber(e: string | number, base?: number): number?",
        "Converts a value to a number when possible. With a base, interprets a string as an integer numeral in that base.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-tonumber",
    ),
    item(
        "tostring",
        LuaItemKind::Function,
        "function tostring(v: any): string",
        "Converts a value to a human-readable string, using __tostring when present.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-tostring",
    ),
    item(
        "type",
        LuaItemKind::Function,
        "function type(v: any): string",
        "Returns the runtime type name: nil, number, string, boolean, table, function, thread, or userdata.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-type",
    ),
    item(
        "unpack",
        LuaItemKind::Function,
        "function unpack(list, i?, j?)",
        "Returns array entries as multiple values.",
    ),
    item(
        "warn",
        LuaItemKind::Function,
        "function warn(message: string, ...)",
        "Emits a warning composed from its string arguments. Lua 5.4 recognizes @off and @on control messages.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-warn",
    ),
    item(
        "xpcall",
        LuaItemKind::Function,
        "function xpcall(f: function, msgh: function, ...): any",
        "Calls a function in protected mode with a message handler.\n\nManual: https://www.lua.org/manual/5.4/manual.html#pdf-xpcall",
    ),
];

const MATH_ITEMS: &[LuaItem] = &[
    item(
        "abs",
        LuaItemKind::Function,
        "function math.abs(x: number): number",
        "Returns the absolute value.",
    ),
    item(
        "acos",
        LuaItemKind::Function,
        "function math.acos(x: number): number",
        "Returns the arc cosine.",
    ),
    item(
        "asin",
        LuaItemKind::Function,
        "function math.asin(x: number): number",
        "Returns the arc sine.",
    ),
    item(
        "atan",
        LuaItemKind::Function,
        "function math.atan(x: number): number",
        "Returns the arc tangent.",
    ),
    item(
        "atan2",
        LuaItemKind::Function,
        "function math.atan2(y: number, x: number): number",
        "Returns the arc tangent using two arguments.",
    ),
    item(
        "ceil",
        LuaItemKind::Function,
        "function math.ceil(x: number): number",
        "Rounds upward.",
    ),
    item(
        "cos",
        LuaItemKind::Function,
        "function math.cos(x: number): number",
        "Returns the cosine.",
    ),
    item(
        "deg",
        LuaItemKind::Function,
        "function math.deg(x: number): number",
        "Converts radians to degrees.",
    ),
    item(
        "floor",
        LuaItemKind::Function,
        "function math.floor(x: number): number",
        "Rounds downward.",
    ),
    item(
        "huge",
        LuaItemKind::Value,
        "number",
        "A value larger than any other numeric value.",
    ),
    item(
        "max",
        LuaItemKind::Function,
        "function math.max(...: number): number",
        "Returns the greatest argument.",
    ),
    item(
        "min",
        LuaItemKind::Function,
        "function math.min(...: number): number",
        "Returns the smallest argument.",
    ),
    item("pi", LuaItemKind::Value, "number", "The value of pi."),
    item(
        "rad",
        LuaItemKind::Function,
        "function math.rad(x: number): number",
        "Converts degrees to radians.",
    ),
    item(
        "random",
        LuaItemKind::Function,
        "function math.random(m?, n?): number",
        "Returns a pseudo-random number.",
    ),
    item(
        "randomseed",
        LuaItemKind::Function,
        "function math.randomseed(seed: number)",
        "Seeds the pseudo-random generator.",
    ),
    item(
        "sin",
        LuaItemKind::Function,
        "function math.sin(x: number): number",
        "Returns the sine.",
    ),
    item(
        "sqrt",
        LuaItemKind::Function,
        "function math.sqrt(x: number): number",
        "Returns the square root.",
    ),
];

const STRING_ITEMS: &[LuaItem] = &[
    item(
        "byte",
        LuaItemKind::Function,
        "function string.byte(s: string, i?, j?)",
        "Returns byte values from a string.",
    ),
    item(
        "char",
        LuaItemKind::Function,
        "function string.char(...: number): string",
        "Builds a string from byte values.",
    ),
    item(
        "find",
        LuaItemKind::Function,
        "function string.find(s: string, pattern: string, init?, plain?)",
        "Finds a pattern in a string.",
    ),
    item(
        "format",
        LuaItemKind::Function,
        "function string.format(format: string, ...): string",
        "Formats values into a string.",
    ),
    item(
        "gmatch",
        LuaItemKind::Function,
        "function string.gmatch(s: string, pattern: string)",
        "Iterates pattern matches.",
    ),
    item(
        "gsub",
        LuaItemKind::Function,
        "function string.gsub(s: string, pattern: string, repl, n?)",
        "Replaces pattern matches.",
    ),
    item(
        "len",
        LuaItemKind::Function,
        "function string.len(s: string): number",
        "Returns string length.",
    ),
    item(
        "lower",
        LuaItemKind::Function,
        "function string.lower(s: string): string",
        "Lowercases a string.",
    ),
    item(
        "match",
        LuaItemKind::Function,
        "function string.match(s: string, pattern: string, init?)",
        "Matches a pattern.",
    ),
    item(
        "rep",
        LuaItemKind::Function,
        "function string.rep(s: string, n: number, sep?)",
        "Repeats a string.",
    ),
    item(
        "reverse",
        LuaItemKind::Function,
        "function string.reverse(s: string): string",
        "Reverses a string.",
    ),
    item(
        "sub",
        LuaItemKind::Function,
        "function string.sub(s: string, i: number, j?): string",
        "Returns a substring.",
    ),
    item(
        "upper",
        LuaItemKind::Function,
        "function string.upper(s: string): string",
        "Uppercases a string.",
    ),
];

const TABLE_ITEMS: &[LuaItem] = &[
    item(
        "concat",
        LuaItemKind::Function,
        "function table.concat(list, sep?, i?, j?): string",
        "Concatenates array entries.",
    ),
    item(
        "insert",
        LuaItemKind::Function,
        "function table.insert(list, pos?, value)",
        "Inserts an array entry.",
    ),
    item(
        "maxn",
        LuaItemKind::Function,
        "function table.maxn(table): number",
        "Returns the largest numeric key in Lua 5.1.",
    ),
    item(
        "remove",
        LuaItemKind::Function,
        "function table.remove(list, pos?)",
        "Removes an array entry.",
    ),
    item(
        "sort",
        LuaItemKind::Function,
        "function table.sort(list, comp?)",
        "Sorts an array-like table.",
    ),
];

const COROUTINE_ITEMS: &[LuaItem] = &[
    item(
        "create",
        LuaItemKind::Function,
        "function coroutine.create(fn): thread",
        "Creates a coroutine.",
    ),
    item(
        "resume",
        LuaItemKind::Function,
        "function coroutine.resume(thread, ...)",
        "Resumes a coroutine.",
    ),
    item(
        "running",
        LuaItemKind::Function,
        "function coroutine.running(): thread?",
        "Returns the running coroutine.",
    ),
    item(
        "status",
        LuaItemKind::Function,
        "function coroutine.status(thread): string",
        "Returns coroutine status.",
    ),
    item(
        "wrap",
        LuaItemKind::Function,
        "function coroutine.wrap(fn): function",
        "Creates a coroutine wrapper function.",
    ),
    item(
        "yield",
        LuaItemKind::Function,
        "function coroutine.yield(...)",
        "Yields from a coroutine.",
    ),
];

const DEBUG_ITEMS: &[LuaItem] = &[
    item(
        "debug",
        LuaItemKind::Function,
        "function debug.debug()",
        "Enters an interactive debug prompt.",
    ),
    item(
        "getinfo",
        LuaItemKind::Function,
        "function debug.getinfo(thread?, target, what?)",
        "Returns debug information.",
    ),
    item(
        "getlocal",
        LuaItemKind::Function,
        "function debug.getlocal(thread?, level, local)",
        "Reads a local variable from a stack frame.",
    ),
    item(
        "setlocal",
        LuaItemKind::Function,
        "function debug.setlocal(thread?, level, local, value)",
        "Writes a local variable in a stack frame.",
    ),
    item(
        "traceback",
        LuaItemKind::Function,
        "function debug.traceback(thread?, message?, level?)",
        "Returns a stack traceback.",
    ),
];

const IO_ITEMS: &[LuaItem] = &[
    item(
        "close",
        LuaItemKind::Function,
        "function io.close(file?)",
        "Closes a file handle.",
    ),
    item(
        "flush",
        LuaItemKind::Function,
        "function io.flush()",
        "Flushes output.",
    ),
    item(
        "input",
        LuaItemKind::Function,
        "function io.input(file?)",
        "Gets or sets the default input file.",
    ),
    item(
        "lines",
        LuaItemKind::Function,
        "function io.lines(filename?)",
        "Iterates lines from a file.",
    ),
    item(
        "open",
        LuaItemKind::Function,
        "function io.open(filename, mode?)",
        "Opens a file.",
    ),
    item(
        "output",
        LuaItemKind::Function,
        "function io.output(file?)",
        "Gets or sets the default output file.",
    ),
    item(
        "read",
        LuaItemKind::Function,
        "function io.read(...)",
        "Reads from the default input file.",
    ),
    item(
        "stderr",
        LuaItemKind::Value,
        "file",
        "Standard error handle.",
    ),
    item(
        "stdin",
        LuaItemKind::Value,
        "file",
        "Standard input handle.",
    ),
    item(
        "stdout",
        LuaItemKind::Value,
        "file",
        "Standard output handle.",
    ),
    item(
        "type",
        LuaItemKind::Function,
        "function io.type(value): string?",
        "Checks whether a value is a file handle.",
    ),
    item(
        "write",
        LuaItemKind::Function,
        "function io.write(...)",
        "Writes to the default output file.",
    ),
];

const OS_ITEMS: &[LuaItem] = &[
    item(
        "clock",
        LuaItemKind::Function,
        "function os.clock(): number",
        "Returns CPU time used by the program.",
    ),
    item(
        "date",
        LuaItemKind::Function,
        "function os.date(format?, time?)",
        "Formats date/time.",
    ),
    item(
        "difftime",
        LuaItemKind::Function,
        "function os.difftime(t2, t1): number",
        "Returns a time difference.",
    ),
    item(
        "execute",
        LuaItemKind::Function,
        "function os.execute(command?)",
        "Executes an operating-system command.",
    ),
    item(
        "exit",
        LuaItemKind::Function,
        "function os.exit(code?)",
        "Terminates the host program.",
    ),
    item(
        "getenv",
        LuaItemKind::Function,
        "function os.getenv(name): string?",
        "Reads an environment variable.",
    ),
    item(
        "remove",
        LuaItemKind::Function,
        "function os.remove(filename)",
        "Removes a file.",
    ),
    item(
        "rename",
        LuaItemKind::Function,
        "function os.rename(oldname, newname)",
        "Renames a file.",
    ),
    item(
        "time",
        LuaItemKind::Function,
        "function os.time(table?): number",
        "Returns a timestamp.",
    ),
];

const PACKAGE_ITEMS: &[LuaItem] = &[
    item(
        "cpath",
        LuaItemKind::Value,
        "string",
        "C module search path.",
    ),
    item(
        "loaded",
        LuaItemKind::Value,
        "table",
        "Loaded module cache.",
    ),
    item(
        "loaders",
        LuaItemKind::Value,
        "table",
        "Lua 5.1 module searchers.",
    ),
    item(
        "path",
        LuaItemKind::Value,
        "string",
        "Lua module search path.",
    ),
    item(
        "preload",
        LuaItemKind::Value,
        "table",
        "Preloaded module functions.",
    ),
    item(
        "seeall",
        LuaItemKind::Function,
        "function package.seeall(module)",
        "Lua 5.1 helper for module environments.",
    ),
];

const LOVE_ITEMS: &[LuaItem] = &[
    item(
        "audio",
        LuaItemKind::Module,
        "LÖVE audio module",
        "Audio playback API.",
    ),
    item(
        "draw",
        LuaItemKind::Function,
        "function love.draw()",
        "Frame drawing callback.",
    ),
    item(
        "event",
        LuaItemKind::Module,
        "LÖVE event module",
        "Event queue API.",
    ),
    item(
        "filesystem",
        LuaItemKind::Module,
        "LÖVE filesystem module",
        "Save and source filesystem API.",
    ),
    item(
        "graphics",
        LuaItemKind::Module,
        "LÖVE graphics module",
        "2D rendering API.",
    ),
    item(
        "keypressed",
        LuaItemKind::Function,
        "function love.keypressed(key, scancode, isrepeat)",
        "Keyboard press callback.",
    ),
    item(
        "keyreleased",
        LuaItemKind::Function,
        "function love.keyreleased(key, scancode)",
        "Keyboard release callback.",
    ),
    item(
        "keyboard",
        LuaItemKind::Module,
        "LÖVE keyboard module",
        "Keyboard input API.",
    ),
    item(
        "load",
        LuaItemKind::Function,
        "function love.load(args)",
        "Startup callback.",
    ),
    item(
        "mouse",
        LuaItemKind::Module,
        "LÖVE mouse module",
        "Mouse input API.",
    ),
    item(
        "mousepressed",
        LuaItemKind::Function,
        "function love.mousepressed(x, y, button, istouch, presses)",
        "Mouse press callback.",
    ),
    item(
        "mousereleased",
        LuaItemKind::Function,
        "function love.mousereleased(x, y, button, istouch, presses)",
        "Mouse release callback.",
    ),
    item(
        "quit",
        LuaItemKind::Function,
        "function love.quit(): boolean?",
        "Quit callback.",
    ),
    item(
        "resize",
        LuaItemKind::Function,
        "function love.resize(width, height)",
        "Window resize callback.",
    ),
    item(
        "timer",
        LuaItemKind::Module,
        "LÖVE timer module",
        "Time and framerate API.",
    ),
    item(
        "update",
        LuaItemKind::Function,
        "function love.update(dt: number)",
        "Frame update callback.",
    ),
    item(
        "window",
        LuaItemKind::Module,
        "LÖVE window module",
        "Window management API.",
    ),
];

const LOVE_AUDIO_ITEMS: &[LuaItem] = &[
    item(
        "getVolume",
        LuaItemKind::Function,
        "function love.audio.getVolume(): number",
        "Gets master volume.",
    ),
    item(
        "newSource",
        LuaItemKind::Function,
        "function love.audio.newSource(filename, type)",
        "Creates an audio source.",
    ),
    item(
        "pause",
        LuaItemKind::Function,
        "function love.audio.pause(source?)",
        "Pauses audio playback.",
    ),
    item(
        "play",
        LuaItemKind::Function,
        "function love.audio.play(source)",
        "Plays an audio source.",
    ),
    item(
        "setVolume",
        LuaItemKind::Function,
        "function love.audio.setVolume(volume: number)",
        "Sets master volume.",
    ),
    item(
        "stop",
        LuaItemKind::Function,
        "function love.audio.stop(source?)",
        "Stops audio playback.",
    ),
];

const LOVE_EVENT_ITEMS: &[LuaItem] = &[
    item(
        "poll",
        LuaItemKind::Function,
        "function love.event.poll()",
        "Iterates pending events.",
    ),
    item(
        "push",
        LuaItemKind::Function,
        "function love.event.push(name, ...)",
        "Pushes an event.",
    ),
    item(
        "quit",
        LuaItemKind::Function,
        "function love.event.quit(exitstatus?)",
        "Queues a quit event.",
    ),
];

const LOVE_FILESYSTEM_ITEMS: &[LuaItem] = &[
    item(
        "createDirectory",
        LuaItemKind::Function,
        "function love.filesystem.createDirectory(name)",
        "Creates a directory.",
    ),
    item(
        "getDirectoryItems",
        LuaItemKind::Function,
        "function love.filesystem.getDirectoryItems(dir): table",
        "Lists directory entries.",
    ),
    item(
        "getInfo",
        LuaItemKind::Function,
        "function love.filesystem.getInfo(path): table?",
        "Gets file metadata.",
    ),
    item(
        "load",
        LuaItemKind::Function,
        "function love.filesystem.load(name)",
        "Loads a Lua chunk from the game filesystem.",
    ),
    item(
        "read",
        LuaItemKind::Function,
        "function love.filesystem.read(name)",
        "Reads a file.",
    ),
    item(
        "write",
        LuaItemKind::Function,
        "function love.filesystem.write(name, data)",
        "Writes a file.",
    ),
];

const LOVE_GRAPHICS_ITEMS: &[LuaItem] = &[
    item(
        "arc",
        LuaItemKind::Function,
        "function love.graphics.arc(mode, x, y, radius, angle1, angle2)",
        "Draws an arc.",
    ),
    item(
        "circle",
        LuaItemKind::Function,
        "function love.graphics.circle(mode, x, y, radius)",
        "Draws a circle.",
    ),
    item(
        "clear",
        LuaItemKind::Function,
        "function love.graphics.clear(...)",
        "Clears the screen.",
    ),
    item(
        "draw",
        LuaItemKind::Function,
        "function love.graphics.draw(drawable, x?, y?, r?, sx?, sy?)",
        "Draws a drawable object.",
    ),
    item(
        "getDimensions",
        LuaItemKind::Function,
        "function love.graphics.getDimensions(): number, number",
        "Returns window dimensions.",
    ),
    item(
        "getHeight",
        LuaItemKind::Function,
        "function love.graphics.getHeight(): number",
        "Returns window height.",
    ),
    item(
        "getWidth",
        LuaItemKind::Function,
        "function love.graphics.getWidth(): number",
        "Returns window width.",
    ),
    item(
        "line",
        LuaItemKind::Function,
        "function love.graphics.line(...)",
        "Draws line segments.",
    ),
    item(
        "newFont",
        LuaItemKind::Function,
        "function love.graphics.newFont(filename?, size?)",
        "Creates a font.",
    ),
    item(
        "newImage",
        LuaItemKind::Function,
        "function love.graphics.newImage(filename)",
        "Creates an image.",
    ),
    item(
        "newQuad",
        LuaItemKind::Function,
        "function love.graphics.newQuad(x, y, w, h, sw, sh)",
        "Creates a quad.",
    ),
    item(
        "pop",
        LuaItemKind::Function,
        "function love.graphics.pop()",
        "Pops the transform stack.",
    ),
    item(
        "print",
        LuaItemKind::Function,
        "function love.graphics.print(text, x?, y?)",
        "Draws text.",
    ),
    item(
        "printf",
        LuaItemKind::Function,
        "function love.graphics.printf(text, x, y, limit, align?)",
        "Draws formatted text.",
    ),
    item(
        "push",
        LuaItemKind::Function,
        "function love.graphics.push(stack?)",
        "Pushes the transform stack.",
    ),
    item(
        "rectangle",
        LuaItemKind::Function,
        "function love.graphics.rectangle(mode, x, y, width, height)",
        "Draws a rectangle.",
    ),
    item(
        "reset",
        LuaItemKind::Function,
        "function love.graphics.reset()",
        "Resets graphics state.",
    ),
    item(
        "rotate",
        LuaItemKind::Function,
        "function love.graphics.rotate(angle)",
        "Rotates the coordinate system.",
    ),
    item(
        "scale",
        LuaItemKind::Function,
        "function love.graphics.scale(sx, sy?)",
        "Scales the coordinate system.",
    ),
    item(
        "setBackgroundColor",
        LuaItemKind::Function,
        "function love.graphics.setBackgroundColor(r, g, b, a?)",
        "Sets background color.",
    ),
    item(
        "setColor",
        LuaItemKind::Function,
        "function love.graphics.setColor(r, g, b, a?)",
        "Sets drawing color.",
    ),
    item(
        "setFont",
        LuaItemKind::Function,
        "function love.graphics.setFont(font)",
        "Sets the active font.",
    ),
    item(
        "setLineWidth",
        LuaItemKind::Function,
        "function love.graphics.setLineWidth(width)",
        "Sets line width.",
    ),
    item(
        "translate",
        LuaItemKind::Function,
        "function love.graphics.translate(dx, dy)",
        "Translates the coordinate system.",
    ),
];

const LOVE_KEYBOARD_ITEMS: &[LuaItem] = &[
    item(
        "hasKeyRepeat",
        LuaItemKind::Function,
        "function love.keyboard.hasKeyRepeat(): boolean",
        "Returns whether key repeat is enabled.",
    ),
    item(
        "isDown",
        LuaItemKind::Function,
        "function love.keyboard.isDown(...): boolean",
        "Checks whether keys are pressed.",
    ),
    item(
        "setKeyRepeat",
        LuaItemKind::Function,
        "function love.keyboard.setKeyRepeat(enable: boolean)",
        "Enables or disables key repeat.",
    ),
];

const LOVE_MOUSE_ITEMS: &[LuaItem] = &[
    item(
        "getPosition",
        LuaItemKind::Function,
        "function love.mouse.getPosition(): number, number",
        "Returns mouse position.",
    ),
    item(
        "isDown",
        LuaItemKind::Function,
        "function love.mouse.isDown(...): boolean",
        "Checks whether mouse buttons are pressed.",
    ),
    item(
        "setVisible",
        LuaItemKind::Function,
        "function love.mouse.setVisible(visible: boolean)",
        "Shows or hides the mouse cursor.",
    ),
];

const LOVE_TIMER_ITEMS: &[LuaItem] = &[
    item(
        "getDelta",
        LuaItemKind::Function,
        "function love.timer.getDelta(): number",
        "Returns the last frame delta time.",
    ),
    item(
        "getFPS",
        LuaItemKind::Function,
        "function love.timer.getFPS(): number",
        "Returns current frames per second.",
    ),
    item(
        "getTime",
        LuaItemKind::Function,
        "function love.timer.getTime(): number",
        "Returns time since an arbitrary point.",
    ),
    item(
        "sleep",
        LuaItemKind::Function,
        "function love.timer.sleep(seconds: number)",
        "Sleeps for a duration.",
    ),
];

const LOVE_WINDOW_ITEMS: &[LuaItem] = &[
    item(
        "getMode",
        LuaItemKind::Function,
        "function love.window.getMode()",
        "Returns window mode settings.",
    ),
    item(
        "getTitle",
        LuaItemKind::Function,
        "function love.window.getTitle(): string",
        "Returns the window title.",
    ),
    item(
        "setMode",
        LuaItemKind::Function,
        "function love.window.setMode(width, height, flags?)",
        "Sets window mode.",
    ),
    item(
        "setTitle",
        LuaItemKind::Function,
        "function love.window.setTitle(title: string)",
        "Sets the window title.",
    ),
];
