use crate::{
    asset, love_api, love_api_extra, love_api_signatures, object_pool, signal, sprite,
    state_machine, store, vector,
};

pub(crate) const BUILTIN_DEFINITION_URI_PREFIX: &str = "tua:/builtins/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LuaItem {
    pub label: &'static str,
    pub kind: LuaItemKind,
    pub detail: &'static str,
    pub documentation: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LuaSignature {
    pub label: &'static str,
    pub documentation: &'static str,
    pub parameters: &'static [LuaSignatureParameter],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LuaSignatureParameter {
    pub label: &'static str,
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
        .chain(ASSET_LOADERS.iter())
        .chain(VECTOR_ITEMS.iter())
        .chain(SIGNAL_ITEMS.iter())
        .chain(OBJECT_POOL_ITEMS.iter())
        .chain(STATE_MACHINE_ITEMS.iter())
        .chain(STORE_ITEMS.iter())
        .chain(SPRITE_ITEMS.iter())
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

pub(crate) fn type_field_items(type_name: &str, prefix: &str) -> Vec<LuaItem> {
    let mut results = love_api_extra::type_field_items(type_name, prefix);

    if let Some(items) = items_for_type(type_name) {
        for item in items
            .iter()
            .copied()
            .filter(|item| item.label.starts_with(prefix))
        {
            push_unique_lua_item(&mut results, item);
        }
    }

    results
}

pub(crate) fn enum_value_items(type_name: &str, prefix: &str) -> Vec<LuaItem> {
    let mut results = love_api_extra::enum_value_items(type_name, prefix);

    if let Some(items) = items_for_enum(type_name) {
        for item in items
            .iter()
            .copied()
            .filter(|item| item.label.starts_with(prefix))
        {
            push_unique_lua_item(&mut results, item);
        }
    }

    results
}

pub(crate) fn love_type_names() -> &'static [&'static str] {
    love_api_extra::type_names()
}

pub(crate) fn love_type_supertypes() -> &'static [(&'static str, &'static str)] {
    love_api_extra::type_supertypes()
}

pub(crate) fn documentation_with_love_wiki_link(item: LuaItem) -> String {
    let mut documentation = item.documentation.to_owned();

    if let Some(url) = love_wiki_url(item) {
        if !documentation.trim().is_empty() {
            documentation.push_str("\n\n");
        }

        documentation.push_str("[Open LOVE wiki](");
        documentation.push_str(&url);
        documentation.push(')');
    }

    documentation
}

pub(crate) fn signature_variants(item: LuaItem) -> Option<&'static [LuaSignature]> {
    let target = function_target(item)?;

    love_api_signatures::signatures_for_target(target)
}

pub(crate) fn love_callback_item(label: &str) -> Option<LuaItem> {
    love_api::callback_item(label)
}

pub(crate) fn love_callback_items() -> Vec<LuaItem> {
    let receiver = vec!["love".to_owned()];

    love_api::field_items(&receiver, "")
        .into_iter()
        .filter(|item| love_api::callback_item(item.label).is_some())
        .collect()
}

pub(crate) fn hover_item(parts: &[String]) -> Option<LuaItem> {
    let (name, receiver) = parts.split_last()?;

    if receiver.is_empty() {
        return TOP_LEVEL
            .iter()
            .chain(ASSET_LOADERS.iter())
            .chain(VECTOR_ITEMS.iter())
            .chain(SIGNAL_ITEMS.iter())
            .chain(OBJECT_POOL_ITEMS.iter())
            .chain(STATE_MACHINE_ITEMS.iter())
            .chain(STORE_ITEMS.iter())
            .chain(SPRITE_ITEMS.iter())
            .find(|item| item.label == name)
            .copied();
    }

    if receiver_is_love(parts) {
        if let Some(item) = love_api::hover_item(parts) {
            return Some(item);
        }
    }

    items_for_receiver(receiver)
        .and_then(|items| items.iter().find(|item| item.label == name).copied())
}

fn love_wiki_url(item: LuaItem) -> Option<String> {
    let page = love_wiki_page(item)?;

    Some(format!("https://love2d.org/wiki/{page}"))
}

fn love_wiki_page(item: LuaItem) -> Option<String> {
    if item.kind == LuaItemKind::Module
        && item.detail.starts_with("LÖVE ")
        && item.detail.ends_with(" module")
        && item.label != "love"
    {
        return Some(format!("love.{}", item.label));
    }

    let target = function_target(item)?;
    if target.starts_with("love.") {
        return Some(target.to_owned());
    }

    let (type_name, method) = target.split_once(':')?;

    if method.is_empty() || !love_type_names().contains(&type_name) {
        return None;
    }

    Some(target.to_owned())
}

fn function_target(item: LuaItem) -> Option<&'static str> {
    item.detail
        .strip_prefix("function ")?
        .split('(')
        .next()
        .map(str::trim)
        .filter(|target| !target.is_empty())
}

pub(crate) fn builtin_definition_for_item(item: LuaItem) -> Option<BuiltinDefinition> {
    let source = builtin_definition_source_for_item(item);
    let span = span_for_label(&source, item.label)?;

    Some(BuiltinDefinition {
        path: builtin_definition_path(item),
        span,
    })
}

pub fn builtin_definition_document(uri: &str) -> Option<String> {
    let id = uri.strip_prefix(BUILTIN_DEFINITION_URI_PREFIX)?;
    let id = id.strip_suffix(".tua").unwrap_or(id);

    all_builtin_items()
        .into_iter()
        .find(|item| builtin_definition_id(*item) == id)
        .map(builtin_definition_source_for_item)
        .or_else(|| love_api::builtin_definition_document(id))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BuiltinDefinition {
    pub path: std::path::PathBuf,
    pub span: crate::SourceSpan,
}

fn items_for_type(type_name: &str) -> Option<&'static [LuaItem]> {
    match type_name {
        "Body" => Some(BODY_ITEMS),
        "Canvas" => Some(CANVAS_ITEMS),
        "Font" => Some(FONT_ITEMS),
        "Image" => Some(IMAGE_ITEMS),
        "ImageData" => Some(IMAGE_DATA_ITEMS),
        "Quad" => Some(QUAD_ITEMS),
        "Shader" => Some(SHADER_ITEMS),
        signal::TYPE_NAME => Some(SIGNAL_BUS_ITEMS),
        object_pool::TYPE_NAME => Some(OBJECT_POOL_METHOD_ITEMS),
        state_machine::TYPE_NAME => Some(STATE_MACHINE_METHOD_ITEMS),
        store::TYPE_NAME => Some(STORE_METHOD_ITEMS),
        "Source" => Some(SOURCE_ITEMS),
        "SpriteBatch" => Some(SPRITE_BATCH_ITEMS),
        "World" => Some(WORLD_ITEMS),
        _ => None,
    }
}

fn all_builtin_items() -> Vec<LuaItem> {
    let mut items = TOP_LEVEL
        .iter()
        .chain(ASSET_LOADERS.iter())
        .chain(VECTOR_ITEMS.iter())
        .chain(SIGNAL_ITEMS.iter())
        .chain(OBJECT_POOL_ITEMS.iter())
        .chain(STATE_MACHINE_ITEMS.iter())
        .chain(STORE_ITEMS.iter())
        .chain(SPRITE_ITEMS.iter())
        .chain(OBJECT_POOL_METHOD_ITEMS.iter())
        .chain(STATE_MACHINE_METHOD_ITEMS.iter())
        .chain(STORE_METHOD_ITEMS.iter())
        .chain(CANVAS_ITEMS.iter())
        .chain(FONT_ITEMS.iter())
        .chain(IMAGE_ITEMS.iter())
        .chain(IMAGE_DATA_ITEMS.iter())
        .chain(QUAD_ITEMS.iter())
        .chain(SHADER_ITEMS.iter())
        .chain(SPRITE_BATCH_ITEMS.iter())
        .chain(BODY_ITEMS.iter())
        .chain(WORLD_ITEMS.iter())
        .chain(DRAW_MODE_ITEMS.iter())
        .chain(FILTER_MODE_ITEMS.iter())
        .chain(MESH_DRAW_MODE_ITEMS.iter())
        .chain(SPRITE_BATCH_USAGE_ITEMS.iter())
        .chain(SOURCE_ITEMS.iter())
        .chain(BIT_ITEMS.iter())
        .chain(MATH_ITEMS.iter())
        .chain(STRING_ITEMS.iter())
        .chain(TABLE_ITEMS.iter())
        .chain(COROUTINE_ITEMS.iter())
        .chain(DEBUG_ITEMS.iter())
        .chain(IO_ITEMS.iter())
        .chain(OS_ITEMS.iter())
        .chain(PACKAGE_ITEMS.iter())
        .copied()
        .collect::<Vec<_>>();

    items.extend(love_api_extra::all_items());

    items
}

fn push_unique_lua_item(items: &mut Vec<LuaItem>, item: LuaItem) {
    if items.iter().any(|existing| existing.label == item.label) {
        return;
    }

    items.push(item);
}

fn builtin_definition_path(item: LuaItem) -> std::path::PathBuf {
    std::path::PathBuf::from(format!(
        "{BUILTIN_DEFINITION_URI_PREFIX}{}.tua",
        builtin_definition_id(item)
    ))
}

pub(crate) fn builtin_definition_id(item: LuaItem) -> String {
    item.detail
        .strip_prefix("function ")
        .and_then(|detail| detail.split_once('(').map(|(name, _)| name))
        .unwrap_or(item.label)
        .replace(':', ".")
}

pub(crate) fn builtin_definition_source_for_item(item: LuaItem) -> String {
    let declaration = if item.detail.starts_with("function ") {
        format!("{} end", item.detail)
    } else {
        format!("local {}: {}", item.label, item.detail)
    };

    format!(
        "-- Tua builtin stub\n-- This virtual document is generated from Tua's builtin Lua/LOVE catalog.\n\n--- {}\n{}\n",
        item.documentation, declaration
    )
}

fn span_for_label(source: &str, label: &str) -> Option<crate::SourceSpan> {
    for (line_index, line) in source.lines().enumerate() {
        if let Some(byte_index) = line.find(label) {
            let column = line[..byte_index].chars().count() + 1;
            let end_column = column + label.chars().count();

            return Some(crate::SourceSpan {
                start: crate::SourcePosition {
                    line: line_index + 1,
                    column,
                },
                end: crate::SourcePosition {
                    line: line_index + 1,
                    column: end_column,
                },
            });
        }
    }

    None
}

fn items_for_enum(type_name: &str) -> Option<&'static [LuaItem]> {
    match type_name {
        "DrawMode" => Some(DRAW_MODE_ITEMS),
        "FilterMode" => Some(FILTER_MODE_ITEMS),
        "MeshDrawMode" => Some(MESH_DRAW_MODE_ITEMS),
        "SpriteBatchUsage" => Some(SPRITE_BATCH_USAGE_ITEMS),
        _ => None,
    }
}

fn receiver_is_love(parts: &[String]) -> bool {
    parts.first().is_some_and(|part| part == "love")
}

fn items_for_receiver(receiver_parts: &[String]) -> Option<&'static [LuaItem]> {
    match receiver_parts {
        [root] if root == "bit" => Some(BIT_ITEMS),
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
        "bit",
        LuaItemKind::Module,
        "LuaJIT bit library",
        "LuaJIT bit operations for 32-bit integers.",
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

const ASSET_LOADERS: &[LuaItem] = &[
    asset_item(asset::AssetLoaderKind::Image),
    asset_item(asset::AssetLoaderKind::Sound),
    asset_item(asset::AssetLoaderKind::Music),
    asset_item(asset::AssetLoaderKind::Font),
    asset_item(asset::AssetLoaderKind::Shader),
    asset_item(asset::AssetLoaderKind::ImageData),
    asset_item(asset::AssetLoaderKind::Canvas),
    asset_item(asset::AssetLoaderKind::Quad),
    asset_item(asset::AssetLoaderKind::SpriteBatch),
    asset_item(asset::AssetLoaderKind::Particles),
];

const VECTOR_ITEMS: &[LuaItem] = &[vector_item()];
const SIGNAL_ITEMS: &[LuaItem] = &[signal_item()];
const OBJECT_POOL_ITEMS: &[LuaItem] = &[object_pool_item()];
const STATE_MACHINE_ITEMS: &[LuaItem] = &[state_machine_item()];
const STORE_ITEMS: &[LuaItem] = &[store_item()];
const SPRITE_ITEMS: &[LuaItem] = &[sprite_item()];

const fn vector_item() -> LuaItem {
    LuaItem {
        label: vector::CONSTRUCTOR_NAME,
        kind: LuaItemKind::Function,
        detail: vector::detail(),
        documentation: vector::documentation(),
    }
}

const fn signal_item() -> LuaItem {
    LuaItem {
        label: signal::CONSTRUCTOR_NAME,
        kind: LuaItemKind::Function,
        detail: signal::detail(),
        documentation: signal::documentation(),
    }
}

const fn object_pool_item() -> LuaItem {
    LuaItem {
        label: object_pool::CONSTRUCTOR_NAME,
        kind: LuaItemKind::Function,
        detail: object_pool::detail(),
        documentation: object_pool::documentation(),
    }
}

const fn state_machine_item() -> LuaItem {
    LuaItem {
        label: state_machine::CONSTRUCTOR_NAME,
        kind: LuaItemKind::Function,
        detail: state_machine::detail(),
        documentation: state_machine::documentation(),
    }
}

const fn store_item() -> LuaItem {
    LuaItem {
        label: store::CONSTRUCTOR_NAME,
        kind: LuaItemKind::Function,
        detail: store::detail(),
        documentation: store::documentation(),
    }
}

const fn sprite_item() -> LuaItem {
    LuaItem {
        label: sprite::HELPER_NAME,
        kind: LuaItemKind::Function,
        detail: sprite::detail(),
        documentation: sprite::documentation(),
    }
}

const SIGNAL_BUS_ITEMS: &[LuaItem] = &[
    item(
        "on",
        LuaItemKind::Function,
        "function SignalBus:on(name: string, handler: function): function",
        "Register a handler for a named signal and return the handler for later removal.",
    ),
    item(
        "once",
        LuaItemKind::Function,
        "function SignalBus:once(name: string, handler: function): function",
        "Register a handler that removes itself after the next matching emit.",
    ),
    item(
        "declare",
        LuaItemKind::Function,
        "function SignalBus:declare(name: string, handler?: function): any",
        "Declare a typed event payload schema without registering runtime behavior.",
    ),
    item(
        "emit",
        LuaItemKind::Function,
        "function SignalBus:emit(name: string, ...: any): nil",
        "Emit a named signal and pass any payload values to each handler.",
    ),
    item(
        "off",
        LuaItemKind::Function,
        "function SignalBus:off(name: string, handler?: function): nil",
        "Remove one handler from a signal, or clear the signal when no handler is provided.",
    ),
    item(
        "clear",
        LuaItemKind::Function,
        "function SignalBus:clear(name?: string): nil",
        "Clear one named signal, or every signal when no name is provided.",
    ),
    item(
        "has",
        LuaItemKind::Function,
        "function SignalBus:has(name: string): boolean",
        "Return true when a named signal has at least one handler.",
    ),
    item(
        "listeners",
        LuaItemKind::Function,
        "function SignalBus:listeners(name: string): table",
        "Return a copy of handlers currently registered for a signal.",
    ),
    item(
        "register",
        LuaItemKind::Function,
        "function SignalBus:register(name: string, handler: function): function",
        "Alias for on, kept for Hump-style signal familiarity.",
    ),
    item(
        "remove",
        LuaItemKind::Function,
        "function SignalBus:remove(name: string, handler?: function): nil",
        "Alias for off, kept for Hump-style signal familiarity.",
    ),
    item(
        "onPattern",
        LuaItemKind::Function,
        "function SignalBus:onPattern(pattern: string, handler: function): function",
        "Register a handler for existing signal names that match a Lua pattern.",
    ),
    item(
        "registerPattern",
        LuaItemKind::Function,
        "function SignalBus:registerPattern(pattern: string, handler: function): function",
        "Alias for onPattern.",
    ),
    item(
        "emitPattern",
        LuaItemKind::Function,
        "function SignalBus:emitPattern(pattern: string, ...: any): nil",
        "Emit every existing signal whose name matches a Lua pattern.",
    ),
    item(
        "offPattern",
        LuaItemKind::Function,
        "function SignalBus:offPattern(pattern: string, handler?: function): nil",
        "Remove a handler from every signal whose name matches a Lua pattern.",
    ),
    item(
        "removePattern",
        LuaItemKind::Function,
        "function SignalBus:removePattern(pattern: string, handler?: function): nil",
        "Alias for offPattern.",
    ),
    item(
        "clearPattern",
        LuaItemKind::Function,
        "function SignalBus:clearPattern(pattern: string): nil",
        "Clear every signal whose name matches a Lua pattern.",
    ),
];

const OBJECT_POOL_METHOD_ITEMS: &[LuaItem] = &[
    item(
        "acquire",
        LuaItemKind::Function,
        "function ObjectPool:acquire(): any",
        "Take an available object or create one when the pool is empty.",
    ),
    item(
        "release",
        LuaItemKind::Function,
        "function ObjectPool:release(object: any): boolean",
        "Reset and return an active object. Returns false for foreign or duplicate releases.",
    ),
    item(
        "prewarm",
        LuaItemKind::Function,
        "function ObjectPool:prewarm(count: number): number",
        "Ensure at least count objects are available and return the number created.",
    ),
    item(
        "clear",
        LuaItemKind::Function,
        "function ObjectPool:clear(): nil",
        "Dispose every owned object and reset the pool to empty.",
    ),
    item(
        "owns",
        LuaItemKind::Function,
        "function ObjectPool:owns(object: any): boolean",
        "Return whether this pool created the object.",
    ),
    item(
        "activeCount",
        LuaItemKind::Function,
        "function ObjectPool:activeCount(): number",
        "Return the number of objects currently checked out.",
    ),
    item(
        "availableCount",
        LuaItemKind::Function,
        "function ObjectPool:availableCount(): number",
        "Return the number of objects ready for immediate reuse.",
    ),
    item(
        "capacity",
        LuaItemKind::Function,
        "function ObjectPool:capacity(): number",
        "Return the total number of objects owned by the pool.",
    ),
];

const STORE_METHOD_ITEMS: &[LuaItem] = &[
    item(
        "get",
        LuaItemKind::Function,
        "function Store:get(): StoreState",
        "Return the current state table.",
    ),
    item(
        "set",
        LuaItemKind::Function,
        "function Store:set(next: StoreState): StoreState",
        "Replace the state table, notify subscribers, and return the new state.",
    ),
    item(
        "update",
        LuaItemKind::Function,
        "function Store:update(mutator: function): StoreState",
        "Mutate the current state, notify subscribers, and return it.",
    ),
    item(
        "subscribe",
        LuaItemKind::Function,
        "function Store:subscribe(listener: function): function",
        "Run a listener after every set or update and return an idempotent unsubscribe function.",
    ),
    item(
        "watch",
        LuaItemKind::Function,
        "function Store:watch(key: string, listener: function): function",
        "Watch one top-level state field and return an idempotent unsubscribe function.",
    ),
];

const STATE_MACHINE_METHOD_ITEMS: &[LuaItem] = &[
    item(
        "is",
        LuaItemKind::Function,
        "function StateMachine:is(state: string): boolean",
        "Return whether the machine is currently in the selected state.",
    ),
    item(
        "can",
        LuaItemKind::Function,
        "function StateMachine:can(event: string): boolean, string | nil",
        "Return whether an event can fire and its destination state.",
    ),
    item(
        "cannot",
        LuaItemKind::Function,
        "function StateMachine:cannot(event: string): boolean",
        "Return whether an event cannot fire from the current state.",
    ),
    item(
        "todot",
        LuaItemKind::Function,
        "function StateMachine:todot(filename: string): nil",
        "Write the configured transition graph as a Graphviz DOT file.",
    ),
    item(
        "transition",
        LuaItemKind::Function,
        "function StateMachine:transition(event: string): boolean | nil",
        "Resume a pending asynchronous transition for the selected event.",
    ),
    item(
        "cancelTransition",
        LuaItemKind::Function,
        "function StateMachine:cancelTransition(event: string): nil",
        "Cancel a pending asynchronous transition for the selected event.",
    ),
];

const CANVAS_ITEMS: &[LuaItem] = &[
    item(
        "clear",
        LuaItemKind::Function,
        "function Canvas:clear(r?: number, g?: number, b?: number, a?: number)",
        "Clears the Canvas.",
    ),
    item(
        "getDimensions",
        LuaItemKind::Function,
        "function Canvas:getDimensions(): number, number",
        "Gets the width and height of the Canvas.",
    ),
    item(
        "getDPIScale",
        LuaItemKind::Function,
        "function Canvas:getDPIScale(): number",
        "Gets the DPI scale factor of the Canvas.",
    ),
    item(
        "getFilter",
        LuaItemKind::Function,
        "function Canvas:getFilter(): FilterMode, FilterMode, number",
        "Gets the filter mode used when scaling the Canvas.",
    ),
    item(
        "getFormat",
        LuaItemKind::Function,
        "function Canvas:getFormat(): PixelFormat",
        "Gets the Canvas texture format.",
    ),
    item(
        "getHeight",
        LuaItemKind::Function,
        "function Canvas:getHeight(): number",
        "Gets the height of the Canvas.",
    ),
    item(
        "getMSAA",
        LuaItemKind::Function,
        "function Canvas:getMSAA(): number",
        "Gets the number of MSAA samples used by the Canvas.",
    ),
    item(
        "getWidth",
        LuaItemKind::Function,
        "function Canvas:getWidth(): number",
        "Gets the width of the Canvas.",
    ),
    item(
        "newImageData",
        LuaItemKind::Function,
        "function Canvas:newImageData(): ImageData",
        "Creates ImageData from the Canvas contents.",
    ),
    item(
        "renderTo",
        LuaItemKind::Function,
        "function Canvas:renderTo(func: function)",
        "Renders to the Canvas for the duration of a function.",
    ),
    item(
        "setFilter",
        LuaItemKind::Function,
        "function Canvas:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)",
        "Sets the filter mode used when scaling the Canvas.",
    ),
    item(
        "setWrap",
        LuaItemKind::Function,
        "function Canvas:setWrap(horizontal: WrapMode, vertical?: WrapMode)",
        "Sets the wrapping properties of the Canvas.",
    ),
];

const FONT_ITEMS: &[LuaItem] = &[
    item(
        "getAscent",
        LuaItemKind::Function,
        "function Font:getAscent(): number",
        "Gets the ascent of the Font.",
    ),
    item(
        "getBaseline",
        LuaItemKind::Function,
        "function Font:getBaseline(): number",
        "Gets the baseline of the Font.",
    ),
    item(
        "getDescent",
        LuaItemKind::Function,
        "function Font:getDescent(): number",
        "Gets the descent of the Font.",
    ),
    item(
        "getFilter",
        LuaItemKind::Function,
        "function Font:getFilter(): FilterMode, FilterMode, number",
        "Gets the Font filter mode.",
    ),
    item(
        "getHeight",
        LuaItemKind::Function,
        "function Font:getHeight(): number",
        "Gets the height of the Font.",
    ),
    item(
        "getLineHeight",
        LuaItemKind::Function,
        "function Font:getLineHeight(): number",
        "Gets the line height scale.",
    ),
    item(
        "getWidth",
        LuaItemKind::Function,
        "function Font:getWidth(text: string): number",
        "Gets the width of a string when rendered with the Font.",
    ),
    item(
        "getWrap",
        LuaItemKind::Function,
        "function Font:getWrap(text: string, wraplimit: number): number, table",
        "Gets wrapped line information for text.",
    ),
    item(
        "hasGlyphs",
        LuaItemKind::Function,
        "function Font:hasGlyphs(text: string): boolean",
        "Gets whether the Font can render every glyph in the text.",
    ),
    item(
        "setFilter",
        LuaItemKind::Function,
        "function Font:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)",
        "Sets the Font filter mode.",
    ),
    item(
        "setLineHeight",
        LuaItemKind::Function,
        "function Font:setLineHeight(height: number)",
        "Sets the line height scale.",
    ),
];

const IMAGE_ITEMS: &[LuaItem] = &[
    item(
        "getDimensions",
        LuaItemKind::Function,
        "function Image:getDimensions(): number, number",
        "Gets the width and height of the Image.",
    ),
    item(
        "getDPIScale",
        LuaItemKind::Function,
        "function Image:getDPIScale(): number",
        "Gets the DPI scale factor of the Image.",
    ),
    item(
        "getFilter",
        LuaItemKind::Function,
        "function Image:getFilter(): FilterMode, FilterMode, number",
        "Gets the filter mode used when scaling the Image.",
    ),
    item(
        "getHeight",
        LuaItemKind::Function,
        "function Image:getHeight(): number",
        "Gets the height of the Image.",
    ),
    item(
        "getMipmapCount",
        LuaItemKind::Function,
        "function Image:getMipmapCount(): number",
        "Gets the number of mipmaps contained in the Image.",
    ),
    item(
        "getPixelDimensions",
        LuaItemKind::Function,
        "function Image:getPixelDimensions(): number, number",
        "Gets the pixel width and height of the Image.",
    ),
    item(
        "getPixelHeight",
        LuaItemKind::Function,
        "function Image:getPixelHeight(): number",
        "Gets the pixel height of the Image.",
    ),
    item(
        "getPixelWidth",
        LuaItemKind::Function,
        "function Image:getPixelWidth(): number",
        "Gets the pixel width of the Image.",
    ),
    item(
        "getWidth",
        LuaItemKind::Function,
        "function Image:getWidth(): number",
        "Gets the width of the Image.",
    ),
    item(
        "getWrap",
        LuaItemKind::Function,
        "function Image:getWrap(): WrapMode, WrapMode",
        "Gets the wrapping properties of the Image.",
    ),
    item(
        "setFilter",
        LuaItemKind::Function,
        "function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)",
        "Sets the filter mode used when scaling the Image.",
    ),
    item(
        "setMipmapFilter",
        LuaItemKind::Function,
        "function Image:setMipmapFilter(filtermode?: FilterMode, sharpness?: number)",
        "Sets the mipmap filter mode for the Image.",
    ),
    item(
        "setWrap",
        LuaItemKind::Function,
        "function Image:setWrap(horizontal: WrapMode, vertical?: WrapMode)",
        "Sets the wrapping properties of the Image.",
    ),
];

const IMAGE_DATA_ITEMS: &[LuaItem] = &[
    item(
        "clone",
        LuaItemKind::Function,
        "function ImageData:clone(): ImageData",
        "Creates a copy of the ImageData.",
    ),
    item(
        "encode",
        LuaItemKind::Function,
        "function ImageData:encode(format: ImageFormat, filename?: string): FileData",
        "Encodes the ImageData to a file format.",
    ),
    item(
        "getDimensions",
        LuaItemKind::Function,
        "function ImageData:getDimensions(): number, number",
        "Gets the width and height of the ImageData.",
    ),
    item(
        "getHeight",
        LuaItemKind::Function,
        "function ImageData:getHeight(): number",
        "Gets the height of the ImageData.",
    ),
    item(
        "getPixel",
        LuaItemKind::Function,
        "function ImageData:getPixel(x: number, y: number): number, number, number, number",
        "Gets the color of a pixel.",
    ),
    item(
        "getWidth",
        LuaItemKind::Function,
        "function ImageData:getWidth(): number",
        "Gets the width of the ImageData.",
    ),
    item(
        "mapPixel",
        LuaItemKind::Function,
        "function ImageData:mapPixel(func: function, x?: number, y?: number, width?: number, height?: number)",
        "Transforms pixels in the ImageData using a function.",
    ),
    item(
        "paste",
        LuaItemKind::Function,
        "function ImageData:paste(source: ImageData, dx: number, dy: number, sx: number, sy: number, sw: number, sh: number)",
        "Pastes another ImageData into this ImageData.",
    ),
    item(
        "setPixel",
        LuaItemKind::Function,
        "function ImageData:setPixel(x: number, y: number, r: number, g: number, b: number, a?: number)",
        "Sets the color of a pixel.",
    ),
];

const QUAD_ITEMS: &[LuaItem] = &[
    item(
        "getLayer",
        LuaItemKind::Function,
        "function Quad:getLayer(): number",
        "Gets the texture layer used by the Quad.",
    ),
    item(
        "getTextureDimensions",
        LuaItemKind::Function,
        "function Quad:getTextureDimensions(): number, number",
        "Gets the reference texture dimensions of the Quad.",
    ),
    item(
        "getViewport",
        LuaItemKind::Function,
        "function Quad:getViewport(): number, number, number, number",
        "Gets the viewport of the Quad.",
    ),
    item(
        "setLayer",
        LuaItemKind::Function,
        "function Quad:setLayer(layer: number)",
        "Sets the texture layer used by the Quad.",
    ),
    item(
        "setViewport",
        LuaItemKind::Function,
        "function Quad:setViewport(x: number, y: number, w: number, h: number, sw?: number, sh?: number)",
        "Sets the viewport of the Quad.",
    ),
];

const SHADER_ITEMS: &[LuaItem] = &[
    item(
        "getWarnings",
        LuaItemKind::Function,
        "function Shader:getWarnings(): string",
        "Gets warnings from shader compilation.",
    ),
    item(
        "hasUniform",
        LuaItemKind::Function,
        "function Shader:hasUniform(name: string): boolean",
        "Gets whether the Shader has a uniform with the given name.",
    ),
    item(
        "send",
        LuaItemKind::Function,
        "function Shader:send(name: string, value: any, ...)",
        "Sends one or more values to a Shader uniform.",
    ),
    item(
        "sendColor",
        LuaItemKind::Function,
        "function Shader:sendColor(name: string, color: table, ...)",
        "Sends colors to a Shader uniform.",
    ),
];

const SPRITE_BATCH_ITEMS: &[LuaItem] = &[
    item(
        "add",
        LuaItemKind::Function,
        "function SpriteBatch:add(x?: number, y?: number, r?: number, sx?: number, sy?: number): number",
        "Adds a sprite to the SpriteBatch.",
    ),
    item(
        "addLayer",
        LuaItemKind::Function,
        "function SpriteBatch:addLayer(layerindex: number, x?: number, y?: number, r?: number, sx?: number, sy?: number): number",
        "Adds a layered sprite to the SpriteBatch.",
    ),
    item(
        "attachAttribute",
        LuaItemKind::Function,
        "function SpriteBatch:attachAttribute(name: string, mesh: Mesh)",
        "Attaches a per-sprite attribute from a Mesh.",
    ),
    item(
        "clear",
        LuaItemKind::Function,
        "function SpriteBatch:clear()",
        "Removes all sprites from the SpriteBatch.",
    ),
    item(
        "flush",
        LuaItemKind::Function,
        "function SpriteBatch:flush()",
        "Immediately uploads pending SpriteBatch changes.",
    ),
    item(
        "getBufferSize",
        LuaItemKind::Function,
        "function SpriteBatch:getBufferSize(): number",
        "Gets the SpriteBatch buffer size.",
    ),
    item(
        "getCount",
        LuaItemKind::Function,
        "function SpriteBatch:getCount(): number",
        "Gets the number of sprites in the SpriteBatch.",
    ),
    item(
        "set",
        LuaItemKind::Function,
        "function SpriteBatch:set(id: number, x?: number, y?: number, r?: number, sx?: number, sy?: number)",
        "Changes a sprite in the SpriteBatch.",
    ),
    item(
        "setColor",
        LuaItemKind::Function,
        "function SpriteBatch:setColor(r?: number, g?: number, b?: number, a?: number)",
        "Sets the color used for new sprites.",
    ),
];

const BODY_ITEMS: &[LuaItem] = &[
    item(
        "applyForce",
        LuaItemKind::Function,
        "function Body:applyForce(x: number, y: number, applyX?: number, applyY?: number)",
        "Applies a force to the Body.",
    ),
    item(
        "applyLinearImpulse",
        LuaItemKind::Function,
        "function Body:applyLinearImpulse(x: number, y: number, applyX?: number, applyY?: number)",
        "Applies a linear impulse to the Body.",
    ),
    item(
        "destroy",
        LuaItemKind::Function,
        "function Body:destroy()",
        "Destroys the Body.",
    ),
    item(
        "getAngle",
        LuaItemKind::Function,
        "function Body:getAngle(): number",
        "Gets the Body angle.",
    ),
    item(
        "getLinearVelocity",
        LuaItemKind::Function,
        "function Body:getLinearVelocity(): number, number",
        "Gets the linear velocity of the Body.",
    ),
    item(
        "getMass",
        LuaItemKind::Function,
        "function Body:getMass(): number",
        "Gets the mass of the Body.",
    ),
    item(
        "getPosition",
        LuaItemKind::Function,
        "function Body:getPosition(): number, number",
        "Gets the position of the Body.",
    ),
    item(
        "getType",
        LuaItemKind::Function,
        "function Body:getType(): BodyType",
        "Gets the Body type.",
    ),
    item(
        "getWorld",
        LuaItemKind::Function,
        "function Body:getWorld(): World",
        "Gets the World the Body belongs to.",
    ),
    item(
        "getX",
        LuaItemKind::Function,
        "function Body:getX(): number",
        "Gets the x position of the Body.",
    ),
    item(
        "getY",
        LuaItemKind::Function,
        "function Body:getY(): number",
        "Gets the y position of the Body.",
    ),
    item(
        "isAwake",
        LuaItemKind::Function,
        "function Body:isAwake(): boolean",
        "Gets whether the Body is awake.",
    ),
    item(
        "setAngle",
        LuaItemKind::Function,
        "function Body:setAngle(angle: number)",
        "Sets the Body angle.",
    ),
    item(
        "setAwake",
        LuaItemKind::Function,
        "function Body:setAwake(awake: boolean)",
        "Sets whether the Body is awake.",
    ),
    item(
        "setLinearVelocity",
        LuaItemKind::Function,
        "function Body:setLinearVelocity(x: number, y: number)",
        "Sets the linear velocity of the Body.",
    ),
    item(
        "setPosition",
        LuaItemKind::Function,
        "function Body:setPosition(x: number, y: number)",
        "Sets the position of the Body.",
    ),
    item(
        "setType",
        LuaItemKind::Function,
        "function Body:setType(type: BodyType)",
        "Sets the Body type.",
    ),
];

const WORLD_ITEMS: &[LuaItem] = &[
    item(
        "destroy",
        LuaItemKind::Function,
        "function World:destroy()",
        "Destroys the World.",
    ),
    item(
        "getBodies",
        LuaItemKind::Function,
        "function World:getBodies(): table",
        "Gets all bodies in the World.",
    ),
    item(
        "getBodyCount",
        LuaItemKind::Function,
        "function World:getBodyCount(): number",
        "Gets the number of bodies in the World.",
    ),
    item(
        "getGravity",
        LuaItemKind::Function,
        "function World:getGravity(): number, number",
        "Gets the gravity of the World.",
    ),
    item(
        "isLocked",
        LuaItemKind::Function,
        "function World:isLocked(): boolean",
        "Gets whether the World is locked during callbacks.",
    ),
    item(
        "queryBoundingBox",
        LuaItemKind::Function,
        "function World:queryBoundingBox(topLeftX: number, topLeftY: number, bottomRightX: number, bottomRightY: number, callback: function)",
        "Calls a function for fixtures inside an axis-aligned bounding box.",
    ),
    item(
        "rayCast",
        LuaItemKind::Function,
        "function World:rayCast(x1: number, y1: number, x2: number, y2: number, callback: function)",
        "Casts a ray through the World and calls a function for each intersection.",
    ),
    item(
        "setCallbacks",
        LuaItemKind::Function,
        "function World:setCallbacks(beginContact?: function, endContact?: function, preSolve?: function, postSolve?: function)",
        "Sets collision callbacks for the World.",
    ),
    item(
        "setGravity",
        LuaItemKind::Function,
        "function World:setGravity(x: number, y: number)",
        "Sets the gravity of the World.",
    ),
    item(
        "update",
        LuaItemKind::Function,
        "function World:update(dt: number)",
        "Updates the World simulation.",
    ),
];

const DRAW_MODE_ITEMS: &[LuaItem] = &[
    item(
        "fill",
        LuaItemKind::Value,
        "DrawMode",
        "Draws a filled shape.",
    ),
    item(
        "line",
        LuaItemKind::Value,
        "DrawMode",
        "Draws only the outline of a shape.",
    ),
];

const FILTER_MODE_ITEMS: &[LuaItem] = &[
    item(
        "linear",
        LuaItemKind::Value,
        "FilterMode",
        "Scales using linear interpolation.",
    ),
    item(
        "nearest",
        LuaItemKind::Value,
        "FilterMode",
        "Scales using nearest-neighbor interpolation.",
    ),
];

const MESH_DRAW_MODE_ITEMS: &[LuaItem] = &[
    item(
        "fan",
        LuaItemKind::Value,
        "MeshDrawMode",
        "Draws vertices as a triangle fan.",
    ),
    item(
        "strip",
        LuaItemKind::Value,
        "MeshDrawMode",
        "Draws vertices as a triangle strip.",
    ),
    item(
        "triangles",
        LuaItemKind::Value,
        "MeshDrawMode",
        "Draws vertices as separate triangles.",
    ),
];

const SPRITE_BATCH_USAGE_ITEMS: &[LuaItem] = &[
    item(
        "dynamic",
        LuaItemKind::Value,
        "SpriteBatchUsage",
        "Optimized for frequent sprite batch changes.",
    ),
    item(
        "static",
        LuaItemKind::Value,
        "SpriteBatchUsage",
        "Optimized for rarely changed sprite batches.",
    ),
    item(
        "stream",
        LuaItemKind::Value,
        "SpriteBatchUsage",
        "Optimized for sprite batch data changed every frame.",
    ),
];

const SOURCE_ITEMS: &[LuaItem] = &[
    item(
        "clone",
        LuaItemKind::Function,
        "function Source:clone(): Source",
        "Creates an identical copy of the Source in the stopped state.",
    ),
    item(
        "getDuration",
        LuaItemKind::Function,
        "function Source:getDuration(unit?: TimeUnit): number",
        "Gets the duration of the Source.",
    ),
    item(
        "getLooping",
        LuaItemKind::Function,
        "function Source:getLooping(): boolean",
        "Gets whether the Source loops.",
    ),
    item(
        "getPitch",
        LuaItemKind::Function,
        "function Source:getPitch(): number",
        "Gets the pitch of the Source.",
    ),
    item(
        "getPosition",
        LuaItemKind::Function,
        "function Source:getPosition(): number, number, number",
        "Gets the position of the Source.",
    ),
    item(
        "getType",
        LuaItemKind::Function,
        "function Source:getType(): SourceType",
        "Gets the type of the Source.",
    ),
    item(
        "getVolume",
        LuaItemKind::Function,
        "function Source:getVolume(): number",
        "Gets the volume of the Source.",
    ),
    item(
        "isLooping",
        LuaItemKind::Function,
        "function Source:isLooping(): boolean",
        "Gets whether the Source loops.",
    ),
    item(
        "isPaused",
        LuaItemKind::Function,
        "function Source:isPaused(): boolean",
        "Gets whether the Source is paused.",
    ),
    item(
        "isPlaying",
        LuaItemKind::Function,
        "function Source:isPlaying(): boolean",
        "Gets whether the Source is playing.",
    ),
    item(
        "pause",
        LuaItemKind::Function,
        "function Source:pause(): boolean",
        "Pauses the Source.",
    ),
    item(
        "play",
        LuaItemKind::Function,
        "function Source:play(): boolean",
        "Starts playing the Source.",
    ),
    item(
        "seek",
        LuaItemKind::Function,
        "function Source:seek(offset: number, unit?: TimeUnit)",
        "Sets the playback position of the Source.",
    ),
    item(
        "setLooping",
        LuaItemKind::Function,
        "function Source:setLooping(loop: boolean)",
        "Sets whether the Source should loop.",
    ),
    item(
        "setPitch",
        LuaItemKind::Function,
        "function Source:setPitch(pitch: number)",
        "Sets the pitch of the Source.",
    ),
    item(
        "setPosition",
        LuaItemKind::Function,
        "function Source:setPosition(x: number, y: number, z: number)",
        "Sets the position of the Source.",
    ),
    item(
        "setVolume",
        LuaItemKind::Function,
        "function Source:setVolume(volume: number)",
        "Sets the volume of the Source.",
    ),
    item(
        "stop",
        LuaItemKind::Function,
        "function Source:stop()",
        "Stops the Source.",
    ),
    item(
        "tell",
        LuaItemKind::Function,
        "function Source:tell(unit?: TimeUnit): number",
        "Gets the current playback position of the Source.",
    ),
];

const fn asset_item(kind: asset::AssetLoaderKind) -> LuaItem {
    LuaItem {
        label: kind.name(),
        kind: LuaItemKind::Function,
        detail: kind.detail(),
        documentation: kind.documentation(),
    }
}

const BIT_ITEMS: &[LuaItem] = &[
    item(
        "arshift",
        LuaItemKind::Function,
        "function bit.arshift(x: number, n: number): number",
        "Arithmetic right shift.",
    ),
    item(
        "band",
        LuaItemKind::Function,
        "function bit.band(...: number): number",
        "Bitwise and.",
    ),
    item(
        "bnot",
        LuaItemKind::Function,
        "function bit.bnot(x: number): number",
        "Bitwise not.",
    ),
    item(
        "bor",
        LuaItemKind::Function,
        "function bit.bor(...: number): number",
        "Bitwise or.",
    ),
    item(
        "bswap",
        LuaItemKind::Function,
        "function bit.bswap(x: number): number",
        "Byte swap.",
    ),
    item(
        "bxor",
        LuaItemKind::Function,
        "function bit.bxor(...: number): number",
        "Bitwise xor.",
    ),
    item(
        "lshift",
        LuaItemKind::Function,
        "function bit.lshift(x: number, n: number): number",
        "Logical left shift.",
    ),
    item(
        "rol",
        LuaItemKind::Function,
        "function bit.rol(x: number, n: number): number",
        "Left rotate.",
    ),
    item(
        "ror",
        LuaItemKind::Function,
        "function bit.ror(x: number, n: number): number",
        "Right rotate.",
    ),
    item(
        "rshift",
        LuaItemKind::Function,
        "function bit.rshift(x: number, n: number): number",
        "Logical right shift.",
    ),
    item(
        "tobit",
        LuaItemKind::Function,
        "function bit.tobit(x: number): number",
        "Normalizes a number to a signed 32-bit integer.",
    ),
    item(
        "tohex",
        LuaItemKind::Function,
        "function bit.tohex(x: number, n?: number): string",
        "Formats a 32-bit integer as hexadecimal.",
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
