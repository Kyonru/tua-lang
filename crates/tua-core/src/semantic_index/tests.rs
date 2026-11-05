use super::*;
use crate::{build_type_environment, lower_parsed_source, parse_tua_source};

#[test]
fn resolves_self_field_to_same_name_type_shape_field() {
    let source = r#"
type Account = {
  name: string,
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:new(name, balance)
  local instance = {
    name = name,
    balance = balance,
  }

  return setmetatable(instance, self)
end

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end
"#;
    let (index, parsed) = semantic_index(source);
    let position = crate::source_text::position_at(source, "self.balance");
    let definition = index
        .definition_at(SourcePosition {
            line: position.line,
            column: position.column + "self.".len(),
        })
        .expect("self.balance should resolve through typed receiver");

    assert_eq!(definition.kind, DefinitionKind::TableField);
    assert_eq!(source_text(&parsed.source, definition.span), "balance");
    assert_eq!(definition.span.start.line, 4);
}

#[test]
fn resolves_self_index_method_call_to_prototype_method() {
    let source = r#"
type Account = {
  name: string,
  balance: number,
}

local Account = {}
Account.__index = Account

function Account:deposit(amount: number): number
  self.balance = self.balance + amount

  return self.balance
end

local account = setmetatable({ name = "checking", balance = 50 }, Account)
account:deposit(25)
"#;
    let (index, parsed) = semantic_index(source);
    let position = crate::source_text::position_at(source, "deposit(25)");
    let definition = index
        .definition_at(position)
        .expect("metatable method call should resolve");

    assert_eq!(definition.kind, DefinitionKind::Function);
    assert_eq!(
        source_text(&parsed.source, definition.span),
        "Account:deposit"
    );
}

#[test]
fn resolves_typed_parameter_field_to_type_shape_field() {
    let source = r#"
type MetaVector = {
  x: number,
  y: number,
  length: number,
}

local function combineVector(left: MetaVector, right: MetaVector): MetaVector
  return {
    x = left.x + right.x,
    y = left.y + right.y,
    length = left.length + right.length,
  }
end
"#;
    let (index, parsed) = semantic_index(source);
    let position = crate::source_text::position_at(source, "right.length");
    let definition = index
        .definition_at(SourcePosition {
            line: position.line,
            column: position.column + "right.".len(),
        })
        .expect("typed parameter field should resolve");

    assert_eq!(definition.kind, DefinitionKind::TableField);
    assert_eq!(source_text(&parsed.source, definition.span), "length");
    assert_eq!(definition.span.start.line, 5);
}

#[test]
fn resolves_inferred_local_table_field_to_constructor_key() {
    let source = r#"
local lookupObject = {
  count = 0,
  y = function(self: any, amount: number): number
    self.count = self.count + amount

    return self.count
  end,
}

lookupObject.count = 1
"#;
    let (index, parsed) = semantic_index(source);
    let receiver = crate::source_text::position_at(source, "lookupObject.count");
    let position = SourcePosition {
        line: receiver.line,
        column: receiver.column + "lookupObject.".len(),
    };
    let definition = index
        .definition_at(position)
        .expect("inferred local table field should resolve to constructor key");

    assert_eq!(definition.kind, DefinitionKind::TableField);
    assert_eq!(source_text(&parsed.source, definition.span), "count");
    assert_eq!(definition.span.start.line, 3);
}

fn semantic_index(source: &str) -> (SemanticIndex, crate::ParsedSource) {
    let parsed = parse_tua_source("src/main.tua", source).expect("source should parse");
    let hir = lower_parsed_source(&parsed);
    let environment = build_type_environment(&parsed).expect("type environment should build");
    let index = SemanticIndex::build(&parsed, Some(&hir), &environment);

    (index, parsed)
}

fn source_text(source: &str, span: SourceSpan) -> &str {
    let line = crate::source_text::line_at(source, span.start.line).expect("line should exist");
    let start = crate::source_text::byte_for_column(line, span.start.column);
    let end = crate::source_text::byte_for_column(line, span.end.column);

    &line[start..end]
}

#[test]
fn catalog_filter_preserves_love_and_domain_navigation() {
    let source = r#"
local image: Image
local source: Source
local pool = objectPool(function() return {active = false} end)
local bus = signal()
love.graphics.rectangle("fill", 0, 0, 10, 10)
image:getWidth()
source:play()
pool:acquire()
bus:emit("damage", 1)
"#;
    let (index, _) = semantic_index(source);
    for needle in [
        "rectangle(\"fill\"",
        "getWidth()",
        "play()",
        "acquire()",
        "emit(\"damage\"",
    ] {
        let position = crate::source_text::position_at(source, needle);
        assert_eq!(
            index
                .definition_at(position)
                .map(|definition| definition.kind),
            Some(DefinitionKind::Builtin),
            "{needle}"
        );
    }
}

#[test]
fn catalog_filter_covers_generated_and_bundled_type_members() {
    for type_name in crate::lua::love_type_names().iter().copied().chain([
        crate::signal::TYPE_NAME,
        crate::object_pool::TYPE_NAME,
        crate::state_machine::TYPE_NAME,
        crate::store::TYPE_NAME,
        crate::input::TYPE_NAME,
    ]) {
        for item in crate::lua::type_field_items(type_name, "")
            .into_iter()
            .filter(|item| item.kind == crate::lua::LuaItemKind::Function)
        {
            assert!(
                crate::lua::could_be_builtin_member(item.label),
                "{type_name}.{}",
                item.label
            );
        }
    }
    assert!(!crate::lua::could_be_builtin_member(
        "snake_following_index"
    ));
    // A custom random:table() call must not resolve its receiver merely
    // because the global table namespace exists in the catalog.
    assert!(!crate::lua::could_be_builtin_member("table"));
}

#[test]
fn precomputed_module_literals_preserve_identifier_targets() {
    let source = r#"
local dep = require("missing_module")
local text = "require('not_a_module')"
-- import("comment_only")
local value: number = 3
print(value)
"#;
    let (index, parsed) = semantic_index(source);
    let position = crate::source_text::position_at(source, "value)");
    let definition = index
        .definition_at(position)
        .expect("local reference target");
    assert_eq!(source_text(&parsed.source, definition.span), "value");
    assert_eq!(definition.span.start.line, 5);
    let call = crate::source_text::position_at(source, "print(value)");
    assert_eq!(
        index.definition_at(call).expect("builtin target").kind,
        DefinitionKind::Builtin
    );
    let module_position = crate::source_text::position_at(source, "missing_module");
    let literal = crate::module_link::module_specifier_at_position(source, module_position)
        .expect("module navigation keeps its shared literal scanner");
    assert_eq!(literal.module, "missing_module");
    assert_eq!(module_specifier_literals(source).len(), 1);
}
