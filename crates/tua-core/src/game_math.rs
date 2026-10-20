use crate::lua::{LuaItem, LuaItemKind};
use crate::scope::{scope_map_parsed_source, ScopeGlobalKind, ScopeResolution};
use crate::{ParsedSource, SourceSpan};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum MathNamespace {
    Math,
    LoveMath,
}

impl MathNamespace {
    fn target(self) -> &'static str {
        match self {
            Self::Math => "math",
            Self::LoveMath => "love.math",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct UsedGameMathHelper {
    pub(crate) namespace: MathNamespace,
    pub(crate) helper: GameMathHelper,
    pub(crate) source_span: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GameMathHelper {
    pub(crate) label: &'static str,
    kind: LuaItemKind,
    math_detail: &'static str,
    love_detail: &'static str,
    documentation: &'static str,
    initializer: &'static str,
}

impl GameMathHelper {
    fn item(self, namespace: MathNamespace) -> LuaItem {
        LuaItem {
            label: self.label,
            kind: self.kind,
            detail: match namespace {
                MathNamespace::Math => self.math_detail,
                MathNamespace::LoveMath => self.love_detail,
            },
            documentation: self.documentation,
        }
    }

    pub(crate) fn definition(self, namespace: MathNamespace) -> String {
        let target = namespace.target();
        format!(
            "{target}.{} = {target}.{} or {}\n",
            self.label, self.label, self.initializer
        )
    }
}

pub(crate) fn items(namespace: MathNamespace, prefix: &str) -> impl Iterator<Item = LuaItem> + '_ {
    HELPERS
        .iter()
        .copied()
        .filter(move |helper| helper.label.starts_with(prefix))
        .map(move |helper| helper.item(namespace))
}

pub(crate) fn item(namespace: MathNamespace, label: &str) -> Option<LuaItem> {
    HELPERS
        .iter()
        .copied()
        .find(|helper| helper.label == label)
        .map(|helper| helper.item(namespace))
}

pub(crate) fn all_items() -> impl Iterator<Item = LuaItem> {
    [MathNamespace::Math, MathNamespace::LoveMath]
        .into_iter()
        .flat_map(|namespace| items(namespace, ""))
}

pub(crate) fn used_helpers(parsed: &ParsedSource) -> Vec<UsedGameMathHelper> {
    let scope = scope_map_parsed_source(parsed);
    let mut used = Vec::new();

    for reference in scope.references() {
        let namespace = match (&*reference.name, &reference.resolution) {
            ("math", ScopeResolution::Global(global))
                if global.kind == ScopeGlobalKind::LuaBuiltin =>
            {
                MathNamespace::Math
            }
            ("love", ScopeResolution::Global(global))
                if global.kind == ScopeGlobalKind::LoveBuiltin =>
            {
                MathNamespace::LoveMath
            }
            _ => continue,
        };
        let Some(offset) =
            crate::source_text::byte_for_position(&parsed.source, reference.span.end)
        else {
            continue;
        };
        let Some(label) = helper_label_after(&parsed.source, offset, namespace) else {
            continue;
        };
        let Some(helper) = HELPERS.iter().copied().find(|helper| helper.label == label) else {
            continue;
        };

        let candidate = UsedGameMathHelper {
            namespace,
            helper,
            source_span: reference.span,
        };
        if !used.iter().any(|existing: &UsedGameMathHelper| {
            existing.namespace == candidate.namespace && existing.helper == candidate.helper
        }) {
            used.push(candidate);
        }
    }

    used.sort_by_key(|used| (used.namespace, used.helper.label));
    used
}

fn helper_label_after(source: &str, mut offset: usize, namespace: MathNamespace) -> Option<&str> {
    if namespace == MathNamespace::LoveMath {
        offset = consume_dot_member(source, offset, "math")?;
    }

    let (label, _) = dot_member(source, offset)?;
    Some(label)
}

fn consume_dot_member(source: &str, offset: usize, expected: &str) -> Option<usize> {
    let (member, end) = dot_member(source, offset)?;
    (member == expected).then_some(end)
}

fn dot_member(source: &str, mut offset: usize) -> Option<(&str, usize)> {
    offset = skip_whitespace(source, offset);
    if !source[offset..].starts_with('.') {
        return None;
    }
    offset += 1;
    offset = skip_whitespace(source, offset);

    let start = offset;
    let mut chars = source[offset..].char_indices();
    let (_, first) = chars.next()?;
    if first != '_' && !first.is_ascii_alphabetic() {
        return None;
    }
    offset += first.len_utf8();
    for (index, character) in chars {
        if character != '_' && !character.is_ascii_alphanumeric() {
            offset = start + index;
            break;
        }
        offset = start + index + character.len_utf8();
    }

    Some((&source[start..offset], offset))
}

fn skip_whitespace(source: &str, mut offset: usize) -> usize {
    while let Some(character) = source[offset..].chars().next() {
        if !character.is_whitespace() {
            break;
        }
        offset += character.len_utf8();
    }
    offset
}

const HELPERS: &[GameMathHelper] = &[
    GameMathHelper {
        label: "tau",
        kind: LuaItemKind::Value,
        math_detail: "number",
        love_detail: "number",
        documentation: "The circle constant tau, equal to two times pi.",
        initializer: "math.pi * 2",
    },
    GameMathHelper {
        label: "clamp",
        kind: LuaItemKind::Function,
        math_detail: "function math.clamp(value: number, minimum: number, maximum: number): number",
        love_detail: "function love.math.clamp(value: number, minimum: number, maximum: number): number",
        documentation: "Clamps a number to an inclusive range.",
        initializer: "function(value, minimum, maximum) return math.max(minimum, math.min(maximum, value)) end",
    },
    GameMathHelper {
        label: "lerp",
        kind: LuaItemKind::Function,
        math_detail: "function math.lerp(from: number, to: number, amount: number): number",
        love_detail: "function love.math.lerp(from: number, to: number, amount: number): number",
        documentation: "Linearly interpolates between two numbers. Amount is not clamped.",
        initializer: "function(from, to, amount) return from + (to - from) * amount end",
    },
    GameMathHelper {
        label: "inverseLerp",
        kind: LuaItemKind::Function,
        math_detail: "function math.inverseLerp(from: number, to: number, value: number): number",
        love_detail: "function love.math.inverseLerp(from: number, to: number, value: number): number",
        documentation: "Returns the unclamped interpolation amount of a value between two endpoints.",
        initializer: "function(from, to, value) return (value - from) / (to - from) end",
    },
    GameMathHelper {
        label: "remap",
        kind: LuaItemKind::Function,
        math_detail: "function math.remap(value: number, inputMinimum: number, inputMaximum: number, outputMinimum: number, outputMaximum: number): number",
        love_detail: "function love.math.remap(value: number, inputMinimum: number, inputMaximum: number, outputMinimum: number, outputMaximum: number): number",
        documentation: "Remaps a number from one range to another without clamping.",
        initializer: "function(value, inputMinimum, inputMaximum, outputMinimum, outputMaximum) return outputMinimum + (value - inputMinimum) / (inputMaximum - inputMinimum) * (outputMaximum - outputMinimum) end",
    },
    GameMathHelper {
        label: "sign",
        kind: LuaItemKind::Function,
        math_detail: "function math.sign(value: number): number",
        love_detail: "function love.math.sign(value: number): number",
        documentation: "Returns -1 for negative numbers, 1 for positive numbers, and 0 for zero.",
        initializer: "function(value) return value < 0 and -1 or value > 0 and 1 or 0 end",
    },
    GameMathHelper {
        label: "round",
        kind: LuaItemKind::Function,
        math_detail: "function math.round(value: number): number",
        love_detail: "function love.math.round(value: number): number",
        documentation: "Rounds a number to the nearest integer, with halves away from zero.",
        initializer: "function(value) return value >= 0 and math.floor(value + 0.5) or math.ceil(value - 0.5) end",
    },
    GameMathHelper {
        label: "smoothstep",
        kind: LuaItemKind::Function,
        math_detail: "function math.smoothstep(edge0: number, edge1: number, value: number): number",
        love_detail: "function love.math.smoothstep(edge0: number, edge1: number, value: number): number",
        documentation: "Interpolates smoothly from 0 to 1 while clamping values outside the edge range.",
        initializer: "function(edge0, edge1, value) local amount = math.max(0, math.min(1, (value - edge0) / (edge1 - edge0))); return amount * amount * (3 - 2 * amount) end",
    },
    GameMathHelper {
        label: "rgb",
        kind: LuaItemKind::Function,
        math_detail: "function math.rgb(red: number, green: number, blue: number): table",
        love_detail: "function love.math.rgb(red: number, green: number, blue: number): table",
        documentation: "Creates an RGB color table by converting 0-255 components to LOVE's 0-1 range.",
        initializer: "function(red, green, blue) return { red / 255, green / 255, blue / 255 } end",
    },
    GameMathHelper {
        label: "rgba",
        kind: LuaItemKind::Function,
        math_detail: "function math.rgba(red: number, green: number, blue: number, alpha: number): table",
        love_detail: "function love.math.rgba(red: number, green: number, blue: number, alpha: number): table",
        documentation: "Creates an RGBA color table by converting 0-255 components to LOVE's 0-1 range.",
        initializer: "function(red, green, blue, alpha) return { red / 255, green / 255, blue / 255, alpha / 255 } end",
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_tua_source;

    #[test]
    fn finds_unshadowed_helpers_in_both_namespaces_once() {
        let parsed = parse_tua_source(
            "src/main.tua",
            r#"local angle = math.tau
local value = math.clamp(12, 0, 10) + math.clamp(4, 0, 10)
local color = love.math.rgba(255, 128, 0, 255)
"#,
        )
        .expect("fixture should parse");

        let used = used_helpers(&parsed);
        assert_eq!(
            used.iter()
                .map(|used| (used.namespace, used.helper.label))
                .collect::<Vec<_>>(),
            vec![
                (MathNamespace::Math, "clamp"),
                (MathNamespace::Math, "tau"),
                (MathNamespace::LoveMath, "rgba"),
            ]
        );
    }

    #[test]
    fn ignores_shadowed_math_and_love_tables() {
        let parsed = parse_tua_source(
            "src/main.tua",
            r#"local math = { clamp = function(value) return value end }
local love = { math = { rgb = function() return {} end } }
return math.clamp(1), love.math.rgb()
"#,
        )
        .expect("fixture should parse");

        assert!(used_helpers(&parsed).is_empty());
    }
}
