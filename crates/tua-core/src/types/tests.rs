use super::*;
use crate::{parse_lua_source, parse_tua_source, DiagnosticCode};

#[test]
fn inferred_dictionary_reads_include_nil_for_editor_queries() {
    let parsed = parse_tua_source("dictionary.tua", "local scores: { [string]: number } = {}\nlocal bracket = scores['missing']\nlocal dotted = scores.missing\n").unwrap();
    let environment = build_type_environment(&parsed).unwrap();
    let semantic = crate::semantic::SemanticModel::new(&parsed, &environment);
    for name in ["bracket", "dotted"] {
        assert_eq!(
            semantic.type_of_name(name, crate::SourcePosition { line: 4, column: 1 }),
            Some(Type::union([Type::number(), Type::nil()]))
        );
    }
}

#[test]
fn primitive_assignability_requires_matching_primitives() {
    assert!(is_assignable(&Type::number(), &Type::number()));
    assert!(is_assignable(&Type::string(), &Type::string()));
    assert!(is_assignable(&Type::boolean(), &Type::boolean()));
    assert!(is_assignable(&Type::nil(), &Type::nil()));

    assert!(!is_assignable(&Type::number(), &Type::string()));
    assert!(!is_assignable(&Type::boolean(), &Type::number()));
    assert!(!is_assignable(&Type::nil(), &Type::number()));
}

#[test]
fn assignability_depth_budget_fails_closed_for_distinct_deep_types() {
    let mut source = Type::string();
    let mut target = Type::number();

    for _ in 0..65 {
        source = Type::array(source);
        target = Type::array(target);
    }

    assert!(!is_assignable(&source, &target));
    assert!(is_assignable(&source, &source));
}

#[test]
fn alias_resolution_depth_budget_preserves_an_unresolved_alias() {
    let mut source = String::new();
    for index in 0..65 {
        source.push_str(&format!("type Alias{index} = Alias{}\n", index + 1));
    }
    source.push_str("type Alias65 = number\n");

    let parsed = parse_tua_source("src/deep-aliases.tua", source)
        .expect("deep acyclic aliases should parse");
    let environment =
        build_type_environment(&parsed).expect("deep acyclic aliases should build an environment");
    let resolved = environment.resolve_aliases(&Type::alias("Alias0"));

    assert_eq!(resolved, Type::alias("Alias64"));
    assert_ne!(resolved, Type::Any);
}

#[test]
fn arrays_parenthesize_union_and_function_item_types() {
    let union = Type::array(Type::union([Type::number(), Type::string()]));
    let function = Type::array(Type::function_returns([Type::number()], [Type::string()]));

    assert_eq!(union.to_string(), "(number | string)[]");
    assert_eq!(function.to_string(), "((number) -> string)[]");
}

#[test]
fn inferred_type_deduplication_is_stable_for_deep_types() {
    let first = Type::Function(FunctionType::new_signature(
        [FunctionParameter::named(
            "left",
            Type::array(Type::table([(
                "position",
                TableField::required(Type::table([
                    ("x", TableField::required(Type::number())),
                    ("y", TableField::required(Type::number())),
                ])),
            )])),
        )],
        1,
        None,
        [Type::boolean()],
    ));
    let equal_with_documentation_name = Type::Function(FunctionType::new_signature(
        [FunctionParameter::named(
            "right",
            Type::array(Type::table([(
                "position",
                TableField::required(Type::table([
                    ("x", TableField::required(Type::number())),
                    ("y", TableField::required(Type::number())),
                ])),
            )])),
        )],
        1,
        None,
        [Type::boolean()],
    ));
    let distinct = Type::string();

    let unique = deduplicate_inferred_types(vec![
        first.clone(),
        equal_with_documentation_name,
        distinct.clone(),
    ]);

    assert_eq!(unique, vec![first, distinct]);
}

#[test]
fn pretty_type_formatting_keeps_short_shapes_inline() {
    let ty = Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::string())),
    ]);

    assert_eq!(
        format_type_with_prefix("local value: ", &ty),
        "local value: { x: number, y: string }"
    );
}

#[test]
fn pretty_type_formatting_breaks_long_shapes_and_unions() {
    let shape = Type::table([
        ("alpha", TableField::required(Type::number())),
        ("beta", TableField::required(Type::string())),
        ("delta", TableField::required(Type::boolean())),
        ("epsilon", TableField::required(Type::number())),
        ("gamma", TableField::required(Type::string())),
    ]);
    let union = Type::union([
        Type::alias("AnimationSequence"),
        Type::alias("EmissionSequence"),
        Type::alias("ParallelSequence"),
        Type::alias("RepeatedSequence"),
    ]);

    assert_eq!(
        format_type_with_prefix("local feedback: ", &shape),
        "local feedback: {\n  alpha: number,\n  beta: string,\n  delta: boolean,\n  epsilon: number,\n  gamma: string\n}"
    );
    assert_eq!(
        format_type_with_prefix("type Step = ", &union),
        "type Step = AnimationSequence\n  | EmissionSequence\n  | ParallelSequence\n  | RepeatedSequence"
    );
}

#[test]
fn pretty_type_formatting_keeps_recursive_depth_bounded() {
    let mut ty = Type::number();

    for _ in 0..100 {
        ty = Type::array(ty);
    }

    assert!(format::format_type_pretty(&ty, 0).contains('…'));
}

#[test]
fn pretty_function_formatting_breaks_at_signature_boundaries() {
    let state = Type::table([
        ("paused", TableField::required(Type::boolean())),
        ("score", TableField::required(Type::number())),
    ]);
    let mutator = Type::Function(FunctionType::new_signature(
        [FunctionParameter::named("state", state.clone())],
        1,
        None,
        [Type::nil()],
    ));

    assert_eq!(
        format_named_function_pretty(
            "Store:update",
            &[FunctionParameter::named("mutator", mutator)],
            false,
            None,
            &[state],
        ),
        "function Store:update(\n  mutator: (state: { paused: boolean, score: number }) -> nil\n): { paused: boolean, score: number }"
    );
}

#[test]
fn class_value_types_do_not_capture_methods_from_prefixed_class_names() {
    let parsed = parse_tua_source(
        "src/classes.tua",
        r#"
final class Beat {
  tick() {
  }
}

final class Beat_Manager {
  reset() {
  }
}
"#,
    )
    .expect("classes should parse");
    let environment = build_type_environment(&parsed).expect("classes should type-check");
    let Type::Table(beat) = environment
        .class_value_type("Beat")
        .expect("class value should exist")
    else {
        panic!("class value should be a table");
    };

    assert!(beat.field("new").is_some());
    assert!(beat.field("tick").is_some());
    assert!(beat.field("Manager_reset").is_none());
}

#[test]
fn any_is_assignable_in_both_directions() {
    assert!(is_assignable(&Type::Any, &Type::number()));
    assert!(is_assignable(&Type::number(), &Type::Any));
    assert!(is_assignable(&Type::Any, &Type::empty_table()));
}

#[test]
fn empty_table_literals_initialize_arrays_and_dictionaries() {
    let empty = Type::empty_table();

    assert!(is_assignable(&empty, &Type::array(Type::number())));
    assert!(is_assignable(
        &empty,
        &Type::dictionary(Type::string(), Type::number())
    ));
}

#[test]
fn luadoc_index_signatures_are_not_required_named_fields() {
    let parsed = parse_lua_source(
        "lib/feel/types/feel.lua",
        r#"---@class FeelPlayOptions
---@field [string] any
---@field restart? boolean
local options = {}
"#,
    )
    .expect("LuaDoc index signature should parse");
    let environment = build_type_environment(&parsed).expect("LuaDoc index signature should lower");
    let options = environment
        .alias("FeelPlayOptions")
        .expect("FeelPlayOptions should be registered");
    let supplied = Type::table([("restart", TableField::required(Type::boolean()))]);

    assert!(is_assignable(&supplied, options));

    let Type::Table(options) = options else {
        panic!("options should remain a table shape");
    };
    assert!(options.semantic_field("[string]").is_none());
    assert!(!options.semantic_fields().contains_key("[string]"));
}

#[test]
fn unions_accept_values_assignable_to_any_member() {
    let number_or_nil = Type::union([Type::number(), Type::nil()]);

    assert!(is_assignable(&Type::number(), &number_or_nil));
    assert!(is_assignable(&Type::nil(), &number_or_nil));
    assert!(!is_assignable(&Type::string(), &number_or_nil));
}

#[test]
fn source_unions_must_have_every_member_assignable_to_target() {
    let numeric = Type::union([Type::number(), Type::nil()]);
    let number_or_string_or_nil = Type::union([Type::number(), Type::string(), Type::nil()]);
    let number_or_nil = Type::union([Type::number(), Type::nil()]);

    assert!(is_assignable(&numeric, &number_or_nil));
    assert!(!is_assignable(&number_or_string_or_nil, &number_or_nil));
}

#[test]
fn aliases_resolve_through_type_context() {
    let context = TypeContext::new().with_alias(TypeAlias::new("HitPoints", Type::number()));

    assert!(context.is_assignable(&Type::alias("HitPoints"), &Type::number()));
    assert!(context.is_assignable(&Type::number(), &Type::alias("HitPoints")));
    assert!(!context.is_assignable(&Type::string(), &Type::alias("HitPoints")));
}

#[test]
fn unknown_aliases_are_only_assignable_to_themselves() {
    assert!(is_assignable(&Type::alias("Vec2"), &Type::alias("Vec2")));
    assert!(!is_assignable(&Type::alias("Vec2"), &Type::alias("Player")));
    assert!(!is_assignable(&Type::alias("Vec2"), &Type::empty_table()));
}

#[test]
fn nominal_supertypes_are_assignable_transitively() {
    let context = TypeContext::new()
        .with_alias(TypeAlias::new("Renderable", Type::alias("Drawable")))
        .with_nominal_supertype("Image", "Texture")
        .with_nominal_supertype("Texture", "Drawable");

    assert!(context.is_assignable(&Type::alias("Image"), &Type::alias("Texture")));
    assert!(context.is_assignable(&Type::alias("Image"), &Type::alias("Drawable")));
    assert!(context.is_assignable(&Type::alias("Image"), &Type::alias("Renderable")));
    assert!(!context.is_assignable(&Type::alias("Drawable"), &Type::alias("Image")));
}

#[test]
fn declared_aliases_do_not_inherit_nominal_type_relations() {
    let context = TypeContext::new()
        .with_alias(TypeAlias::new("Image", Type::string()))
        .with_nominal_supertype("Image", "Drawable");

    assert!(!context.is_assignable(&Type::alias("Image"), &Type::alias("Drawable")));
}

#[test]
fn class_aliases_can_opt_into_nominal_inheritance() {
    let mut context = TypeContext::new()
        .with_alias(TypeAlias::new("Entity", Type::empty_table()))
        .with_alias(TypeAlias::new("Particle", Type::empty_table()));
    context.insert_nominal_alias_supertype("Particle", "Entity");

    assert!(context.is_assignable(&Type::alias("Particle"), &Type::alias("Entity")));
    assert!(!context.is_assignable(&Type::alias("Entity"), &Type::alias("Particle")));
}

#[test]
fn class_alias_targets_reject_structurally_matching_plain_tables() {
    let shape = Type::table([("x", TableField::required(Type::number()))]);
    let mut context = TypeContext::new().with_alias(TypeAlias::new("Entity", shape.clone()));
    context.insert_nominal_alias("Entity");

    assert!(!context.is_assignable(&shape, &Type::alias("Entity")));
    assert!(context.is_assignable(&Type::alias("Entity"), &shape));
}

#[test]
fn table_shape_compatibility_is_structural() {
    let source = Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
        ("debug_name", TableField::required(Type::string())),
    ]);
    let target = Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
    ]);

    assert!(is_assignable(&source, &target));
    assert!(!is_assignable(&target, &source));
}

#[test]
fn any_table_value_is_assignable_to_a_plain_table() {
    let plain = Type::empty_table();

    assert!(is_assignable(&Type::array(Type::number()), &plain));
    assert!(is_assignable(
        &Type::dictionary(Type::string(), Type::number()),
        &plain
    ));
    assert!(!is_assignable(&Type::number(), &plain));
    assert!(!is_assignable(
        &Type::array(Type::number()),
        &Type::table([("x", TableField::required(Type::number()))])
    ));
}

#[test]
fn widening_literals_collapses_members_that_become_equal() {
    let row = |id: &str| Type::table([("id", TableField::required(Type::string_literal(id)))]);

    assert_eq!(
        widen_inferred_type(&Type::union([row("a"), row("b")])),
        Type::table([("id", TableField::required(Type::string()))])
    );
    assert_eq!(
        widen_inferred_type(&Type::union([
            Type::string_literal("a"),
            Type::string_literal("b"),
            Type::nil(),
        ])),
        Type::union([Type::string(), Type::nil()])
    );
}

#[test]
fn record_rows_merge_with_optional_fields_but_other_items_stay_distinct() {
    let merged = merge_record_rows(vec![
        Type::table([("id", TableField::required(Type::string()))]),
        Type::table([
            ("id", TableField::required(Type::string())),
            ("level", TableField::required(Type::number())),
        ]),
    ]);
    assert_eq!(
        merged,
        vec![Type::table([
            ("id", TableField::required(Type::string())),
            ("level", TableField::optional(Type::number())),
        ])]
    );

    let mixed = vec![
        Type::table([("id", TableField::required(Type::string()))]),
        Type::number(),
    ];
    assert_eq!(merge_record_rows(mixed.clone()), mixed);
}

#[test]
fn table_fields_must_be_assignable() {
    let source = Type::table([("hp", TableField::required(Type::string()))]);
    let target = Type::table([("hp", TableField::required(Type::number()))]);

    assert!(!is_assignable(&source, &target));
}

#[test]
fn assignability_failures_preserve_nested_dictionary_field_paths() {
    let context = TypeContext::new()
        .with_alias(TypeAlias::new(
            "Animation",
            Type::table([("row", TableField::required(Type::number()))]),
        ))
        .with_alias(TypeAlias::new(
            "Config",
            Type::table([(
                "animations",
                TableField::required(Type::dictionary(Type::string(), Type::alias("Animation"))),
            )]),
        ));
    let supplied = Type::table([(
        "animations",
        TableField::required(Type::table([(
            "green",
            TableField::required(Type::table([("row", TableField::required(Type::string()))])),
        )])),
    )]);

    let failure = context
        .assignability_failure(&supplied, &Type::alias("Config"))
        .expect("the nested row type should be incompatible");

    assert_eq!(
        failure.path,
        vec![
            AssignabilityPathSegment::Field("animations".to_owned()),
            AssignabilityPathSegment::Field("green".to_owned()),
            AssignabilityPathSegment::Field("row".to_owned()),
        ]
    );
    assert_eq!(failure.expected, Type::number());
    assert_eq!(failure.found, Type::string());
    assert_eq!(failure.kind, AssignabilityFailureKind::TypeMismatch);
}

#[test]
fn assignability_failures_identify_callback_returns() {
    let supplied = Type::function_returns([Type::number()], [Type::string()]);
    let expected = Type::function_returns([Type::number()], [Type::number()]);

    let failure = TypeContext::new()
        .assignability_failure(&supplied, &expected)
        .expect("the callback return type should be incompatible");

    assert_eq!(
        failure.path,
        vec![AssignabilityPathSegment::FunctionReturn(0)]
    );
    assert_eq!(failure.expected, Type::number());
    assert_eq!(failure.found, Type::string());
}

#[test]
fn assignability_failures_orient_callback_parameters_to_the_expected_contract() {
    let supplied = Type::Function(FunctionType::new_signature(
        [FunctionParameter::named("value", Type::string())],
        1,
        None,
        [Type::nil()],
    ));
    let expected = Type::Function(FunctionType::new_signature(
        [FunctionParameter::named("value", Type::number())],
        1,
        None,
        [Type::nil()],
    ));

    let failure = TypeContext::new()
        .assignability_failure(&supplied, &expected)
        .expect("the callback parameter type should be incompatible");

    assert_eq!(
        failure.path,
        vec![AssignabilityPathSegment::FunctionParameter {
            index: 0,
            name: Some("value".to_owned()),
        }]
    );
    assert_eq!(failure.expected, Type::number());
    assert_eq!(failure.found, Type::string());
}

#[test]
fn optional_target_fields_may_be_missing() {
    let source = Type::table([("level", TableField::required(Type::number()))]);
    let target = Type::table([
        ("level", TableField::required(Type::number())),
        ("name", TableField::optional(Type::string())),
    ]);

    assert!(is_assignable(&source, &target));
}

#[test]
fn optional_source_fields_do_not_satisfy_required_target_fields() {
    let source = Type::table([("name", TableField::optional(Type::string()))]);
    let target = Type::table([("name", TableField::required(Type::string()))]);

    assert!(!is_assignable(&source, &target));
}

#[test]
fn optional_fields_still_check_present_value_types() {
    let source = Type::table([("name", TableField::optional(Type::number()))]);
    let target = Type::table([("name", TableField::optional(Type::string()))]);

    assert!(!is_assignable(&source, &target));
}

#[test]
fn function_types_are_contravariant_in_parameters_and_covariant_in_return() {
    let any_to_number = Type::function([Type::Any], Type::number());
    let number_to_number = Type::function([Type::number()], Type::number());
    let string_to_number = Type::function([Type::string()], Type::number());
    let any_to_string = Type::function([Type::Any], Type::string());

    assert!(is_assignable(&any_to_number, &number_to_number));
    assert!(!is_assignable(&string_to_number, &number_to_number));
    assert!(!is_assignable(&any_to_string, &number_to_number));
}

#[test]
fn string_literals_are_assignable_to_string_and_exact_literal_targets() {
    let mode = Type::union([
        Type::string_literal("static"),
        Type::string_literal("stream"),
    ]);

    assert!(is_assignable(
        &Type::string_literal("static"),
        &Type::string()
    ));
    assert!(is_assignable(&Type::string_literal("static"), &mode));
    assert!(!is_assignable(&Type::string_literal("dynamic"), &mode));
    assert!(!is_assignable(&Type::string(), &mode));
}

#[test]
fn container_types_are_assignable_by_shape() {
    assert!(is_assignable(
        &Type::array(Type::alias("Image")),
        &Type::array(Type::alias("Image")),
    ));
    assert!(is_assignable(
        &Type::tuple([Type::number(), Type::number()]),
        &Type::tuple([Type::number(), Type::number()]),
    ));
    assert!(is_assignable(
        &Type::dictionary(Type::string(), Type::alias("Source")),
        &Type::dictionary(Type::string(), Type::alias("Source")),
    ));

    let record = Type::table([
        ("idle", TableField::required(Type::number())),
        ("running", TableField::required(Type::number())),
    ]);
    assert!(is_assignable(
        &record,
        &Type::dictionary(Type::string(), Type::number()),
    ));
    assert!(!is_assignable(
        &record,
        &Type::dictionary(Type::string(), Type::boolean()),
    ));
    assert!(!is_assignable(
        &record,
        &Type::dictionary(Type::number(), Type::number()),
    ));
}

#[test]
fn extracts_metatable_facts_from_table_shape() {
    let index_shape = TableShape::from_fields([("length", TableField::required(Type::number()))]);
    let add = FunctionType::new_returns([Type::Any, Type::Any], [Type::Any]);
    let sub = FunctionType::new_returns([Type::Any, Type::Any], [Type::Any]);
    let mul = FunctionType::new_returns([Type::Any, Type::Any], [Type::Any]);
    let div = FunctionType::new_returns([Type::Any, Type::Any], [Type::Any]);
    let eq = FunctionType::new([Type::Any, Type::Any], Type::boolean());
    let lt = FunctionType::new([Type::Any, Type::Any], Type::boolean());
    let le = FunctionType::new([Type::Any, Type::Any], Type::boolean());
    let len = FunctionType::new([Type::Any], Type::number());
    let call = FunctionType::new([Type::Any, Type::number()], Type::Any);
    let tostring = FunctionType::new([Type::Any], Type::string());
    let newindex = FunctionType::new([Type::Any, Type::string(), Type::Any], Type::nil());
    let meta = Type::table([
        (
            "__index",
            TableField::required(Type::Table(index_shape.clone())),
        ),
        (
            "__newindex",
            TableField::required(Type::Function(newindex.clone())),
        ),
        ("__add", TableField::required(Type::Function(add.clone()))),
        ("__sub", TableField::required(Type::Function(sub.clone()))),
        ("__mul", TableField::required(Type::Function(mul.clone()))),
        ("__div", TableField::required(Type::Function(div.clone()))),
        ("__eq", TableField::required(Type::Function(eq.clone()))),
        ("__lt", TableField::required(Type::Function(lt.clone()))),
        ("__le", TableField::required(Type::Function(le.clone()))),
        ("__len", TableField::required(Type::Function(len.clone()))),
        ("__call", TableField::required(Type::Function(call.clone()))),
        (
            "__tostring",
            TableField::required(Type::Function(tostring.clone())),
        ),
    ]);

    let facts = MetatableFacts::from_type(&meta);

    assert_eq!(facts.index, IndexFallback::Table(index_shape));
    assert_eq!(facts.newindex, NewIndexRule::Function(newindex));
    assert_eq!(facts.add, Some(add));
    assert_eq!(facts.sub, Some(sub));
    assert_eq!(facts.mul, Some(mul));
    assert_eq!(facts.div, Some(div));
    assert_eq!(facts.eq, Some(eq));
    assert_eq!(facts.lt, Some(lt));
    assert_eq!(facts.le, Some(le));
    assert_eq!(facts.len, Some(len));
    assert_eq!(facts.call, Some(call));
    assert_eq!(facts.tostring, Some(tostring));
}

#[test]
fn newindex_rules_resolve_assignment_types_for_known_keys() {
    let table_rule = NewIndexRule::Table(TableShape::from_fields([(
        "label",
        TableField::required(Type::string()),
    )]));
    let function_rule = NewIndexRule::Function(FunctionType::new(
        [Type::Any, Type::string_literal("label"), Type::string()],
        Type::nil(),
    ));

    assert_eq!(
        table_rule.assignment_type_for_key("label"),
        Some(Type::string())
    );
    assert_eq!(
        function_rule.assignment_type_for_key("label"),
        Some(Type::string())
    );
    assert_eq!(function_rule.assignment_type_for_key("count"), None);
}

#[test]
fn attaches_metatable_facts_without_polluting_base_fields() {
    let base = Type::table([
        ("x", TableField::required(Type::number())),
        ("length", TableField::required(Type::string())),
    ]);
    let index_shape = TableShape::from_fields([
        ("length", TableField::required(Type::number())),
        ("label", TableField::required(Type::string())),
    ]);
    let add = FunctionType::new_returns([Type::Any, Type::Any], [Type::Any]);
    let meta = Type::table([
        ("__index", TableField::required(Type::Table(index_shape))),
        ("__add", TableField::required(Type::Function(add.clone()))),
    ]);

    let applied = MetatableFacts::from_type(&meta).apply_to_type(base);
    let Type::Table(table) = applied else {
        panic!("metatable facts should preserve table shape");
    };

    assert_eq!(
        table.field("x").map(|field| &field.ty),
        Some(&Type::number())
    );
    assert_eq!(
        table.field("length").map(|field| &field.ty),
        Some(&Type::string())
    );
    assert!(table.field("label").is_none());
    assert!(table.field("__add").is_none());
    assert_eq!(
        table.semantic_field("label").map(|field| &field.ty),
        Some(&Type::string())
    );
    assert_eq!(
        table.semantic_field("length").map(|field| &field.ty),
        Some(&Type::string())
    );
    assert_eq!(
        table.metatable_facts().and_then(|facts| facts.add.as_ref()),
        Some(&add)
    );
}

#[test]
fn metatable_facts_expose_implied_fields() {
    let index_shape = TableShape::from_fields([("length", TableField::required(Type::number()))]);
    let meta = Type::table([
        ("__index", TableField::required(Type::Table(index_shape))),
        ("label", TableField::required(Type::string())),
    ]);

    let facts = MetatableFacts::from_type(&meta);

    assert_eq!(
        facts.implied_field("length").map(|field| &field.ty),
        Some(&Type::number())
    );
    assert_eq!(
        facts.fields.field("label").map(|field| &field.ty),
        Some(&Type::string())
    );
    assert!(facts.implied_field("label").is_none());
    assert!(facts.implied_field("missing").is_none());
}

#[test]
fn semantic_fields_include_index_fields_but_hide_metamethods() {
    let index_shape = TableShape::from_fields([("length", TableField::required(Type::number()))]);
    let add = FunctionType::new_returns([Type::Any, Type::Any], [Type::Any]);
    let meta = Type::table([
        ("__index", TableField::required(Type::Table(index_shape))),
        ("__add", TableField::required(Type::Function(add))),
    ]);
    let applied = MetatableFacts::from_type(&meta)
        .apply_to_type(Type::table([("x", TableField::required(Type::number()))]));
    let Type::Table(table) = applied else {
        panic!("metatable facts should preserve table shape");
    };
    let fields = table.semantic_fields();

    assert!(fields.contains_key("x"));
    assert!(fields.contains_key("length"));
    assert!(!fields.contains_key("__add"));
    assert!(!fields.contains_key("__index"));
}

#[test]
fn semantic_fields_follow_metatable_index_chains() {
    let base = TableShape::from_fields([(
        "describe",
        TableField::required(Type::function([], Type::string())),
    )]);
    let mid_meta = Type::table([("__index", TableField::required(Type::Table(base)))]);
    let mid = TableShape::from_fields([("level", TableField::required(Type::number()))])
        .with_metatable_facts(MetatableFacts::from_type(&mid_meta));
    let leaf_meta = Type::table([("__index", TableField::required(Type::Table(mid)))]);
    let leaf = TableShape::new().with_metatable_facts(MetatableFacts::from_type(&leaf_meta));
    let fields = leaf.semantic_fields();

    assert!(fields.contains_key("describe"), "{fields:?}");
    assert!(fields.contains_key("level"), "{fields:?}");
    assert_eq!(
        leaf.semantic_field("describe").map(|field| &field.ty),
        Some(&Type::function([], Type::string()))
    );
}

#[test]
fn metatable_facts_extract_iterator_metamethods() {
    let pairs = FunctionType::new_returns([Type::Any], [Type::string(), Type::number()]);
    let ipairs = FunctionType::new_returns([Type::Any], [Type::number(), Type::string()]);
    let meta = Type::table([
        (
            "__pairs",
            TableField::required(Type::Function(pairs.clone())),
        ),
        (
            "__ipairs",
            TableField::required(Type::Function(ipairs.clone())),
        ),
    ]);
    let facts = MetatableFacts::from_type(&meta);

    assert_eq!(facts.pairs, Some(pairs));
    assert_eq!(facts.ipairs, Some(ipairs));
}

#[test]
fn builds_per_file_type_environment_from_aliases_and_annotations() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Vec2 = {
  x: number,
  y: number,
}

type Player = {
  pos: Vec2,
  name?: string,
}

local player: Player = nil

local function move(player: Player, delta: Vec2): Player
  return player
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should collect annotations");

    assert_eq!(environment.path(), parsed.path.as_path());
    assert_eq!(environment.alias("Vec2"), Some(&vec2_type()));
    assert_eq!(environment.local("player"), Some(&Type::alias("Player")));

    let functions = environment.functions();
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name.as_deref(), Some("move"));
    assert_eq!(functions[0].parameter_count, 2);
    assert!(!functions[0].has_varargs);
    assert!(functions[0].has_fully_annotated_parameters());
    assert_eq!(functions[0].parameters[0].name, "player");
    assert_eq!(functions[0].parameters[0].ty, Type::alias("Player"));
    assert_eq!(functions[0].parameters[1].name, "delta");
    assert_eq!(functions[0].parameters[1].ty, Type::alias("Vec2"));
    assert_eq!(functions[0].return_type, Some(Type::alias("Player")));
}

#[test]
fn injects_self_type_for_colon_method_annotations() {
    let parsed = parse_tua_source(
        "src/account.tua",
        r#"
type Account = {
  balance: number,
}

function Account:withdraw(amount: number): number
  self.balance -= amount
  return self.balance
end
"#,
    )
    .expect("typed method source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should collect annotations");
    let function = environment
        .function("Account:withdraw")
        .expect("colon method should be collected");

    assert_eq!(function.parameter_count, 2);
    assert!(function.has_fully_annotated_parameters());
    assert_eq!(function.parameters[0].name, "self");
    assert_eq!(function.parameters[0].ty, Type::alias("Account"));
    assert_eq!(function.parameters[1].name, "amount");
    assert_eq!(function.parameters[1].ty, Type::number());
    assert_eq!(function.return_type, Some(Type::number()));
}

#[test]
fn resolves_self_type_in_alias_and_colon_method_annotations() {
    let parsed = parse_tua_source(
        "src/account.tua",
        r#"
type Node = {
  parent: Self | nil,
}

type Account = {
  balance: number,
}

function Account:merge(other: Self): Self
  return self
end
"#,
    )
    .expect("Self source should parse");
    let environment =
        build_type_environment(&parsed).expect("Self should resolve in scoped annotations");

    let Type::Table(node) = environment.alias("Node").expect("Node alias should exist") else {
        panic!("Node should lower to a table shape");
    };

    assert_eq!(
        node.field("parent").map(|field| &field.ty),
        Some(&Type::union([Type::alias("Node"), Type::nil()]))
    );

    let function = environment
        .function("Account:merge")
        .expect("Self method should be collected");

    assert_eq!(function.parameters[0].ty, Type::alias("Account"));
    assert_eq!(function.parameters[1].ty, Type::alias("Account"));
    assert_eq!(function.return_type, Some(Type::alias("Account")));
}

#[test]
fn resolves_self_type_in_lua_doc_class_and_method_annotations() {
    let parsed = parse_lua_source(
        "src/account.lua",
        r#"---@class Account
---@field balance number
---@field peer Self
local Account = {}

---@param other Self
---@return Self
function Account:merge(other)
  return self
end
"#,
    )
    .expect("LuaDoc Self source should parse");
    let environment =
        build_type_environment(&parsed).expect("LuaDoc Self should resolve in scoped annotations");

    let Type::Table(account) = environment
        .alias("Account")
        .expect("Account alias should exist")
    else {
        panic!("Account should lower to a table shape");
    };

    assert_eq!(
        account.field("peer").map(|field| &field.ty),
        Some(&Type::alias("Account"))
    );

    let function = environment
        .function("Account:merge")
        .expect("LuaDoc Self method should be collected");

    assert_eq!(function.parameters[0].ty, Type::alias("Account"));
    assert_eq!(function.return_type, Some(Type::alias("Account")));
}

#[test]
fn resolves_self_type_inside_native_class_methods() {
    let parsed = parse_tua_source(
        "src/classes.tua",
        r#"
final class Node {
  parent: Self | nil

  init(parent: Self | nil) {
    self.parent = parent
  }

  attach(child: Self): Self {
    child.parent = self
    return child
  }
}
"#,
    )
    .expect("class source should parse");
    let environment = build_type_environment(&parsed).expect("Self should resolve in classes");
    let Type::Table(node) = environment.alias("Node").expect("Node alias should exist") else {
        panic!("Node should resolve to a class shape");
    };
    let Type::Function(attach) = &node.field("attach").expect("attach should exist").ty else {
        panic!("attach should retain its function signature");
    };

    assert_eq!(attach.parameters[1].ty, Type::alias("Node"));
    assert_eq!(attach.return_types_or_any(), &[Type::alias("Node")]);
}

#[test]
fn infers_function_return_types_from_annotated_parameters() {
    let parsed = parse_tua_source(
        "src/functions.tua",
        r#"
type Player = { hp: number }

local function identity(value: string)
  return value
end

function getHp(player: Player)
  return player.hp
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer returns");

    assert_eq!(
        environment
            .function("identity")
            .and_then(|function| function.return_type.clone()),
        Some(Type::string())
    );
    assert_eq!(
        environment
            .function("getHp")
            .and_then(|function| function.return_type.clone()),
        Some(Type::number())
    );
}

#[test]
fn infers_function_return_types_with_flow_sensitive_narrowing() {
    let parsed = parse_tua_source(
        "src/narrowing.tua",
        r#"
local function fromOptional(value: number | nil)
  if value ~= nil then
    return value
  end

  return 0
end

local function fromTypeGuard(value: string | number)
  if type(value) == "string" then
    return value
  end

  return ""
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer narrowed returns");

    assert_eq!(
        environment
            .function("fromOptional")
            .and_then(|function| function.return_type.clone()),
        Some(Type::number())
    );
    assert_eq!(
        environment
            .function("fromTypeGuard")
            .and_then(|function| function.return_type.clone()),
        Some(Type::string())
    );
}

#[test]
fn infers_branched_multi_return_slots_without_nil() {
    let parsed = parse_tua_source(
        "src/normalize.tua",
        r#"
local function normalize(dx: number, dy: number)
  local length = math.sqrt(dx * dx + dy * dy)
  if length <= 0.0001 then
    return 0, 0
  end
  return dx / length, dy / length
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer returns");
    let normalize = environment
        .function("normalize")
        .expect("normalize should have an inferred function type");

    assert_eq!(normalize.return_types, vec![Type::number(), Type::number()]);
}

#[test]
fn infers_rawget_array_results_as_optional() {
    let parsed = parse_tua_source(
        "src/safe_index.tua",
        r#"
local function safeIndex(values: number[], index: number)
  return rawget(values, index)
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer returns");
    let safe_index = environment
        .function("safeIndex")
        .expect("safeIndex should have an inferred function type");

    assert_eq!(
        safe_index.return_types,
        vec![Type::union([Type::number(), Type::nil()])]
    );
}

#[test]
fn infers_love_method_function_returns_from_inferred_locals() {
    let parsed = parse_tua_source(
        "src/love_returns.tua",
        r#"
local function loadPlayerImage()
  return image "assets/player.png"
end

local player = loadPlayerImage()

local function azul()
  return player.getDimensions()
end

local c = azul()

local function sumWidth()
  return c + 1
end
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("type environment should infer Love returns");

    let azul = environment
        .function("azul")
        .expect("azul should have an inferred function type");

    assert_eq!(azul.return_type, Some(Type::number()));
    assert_eq!(azul.return_types, vec![Type::number(), Type::number()]);
    assert_eq!(
        environment
            .function("sumWidth")
            .and_then(|function| function.return_type.clone()),
        Some(Type::number())
    );
}

#[test]
fn lowers_lua_doc_annotations_into_type_environment() {
    let parsed = parse_lua_source(
        "src/player.lua",
        r#"---@class Player player object
---@field hp number current health
---@field name? string display name
---@alias PlayerId string|number stable id
---@type Player
Player = {}

---@type number global score
Score = 0

---@param name string player name
---@return Player created player
function makePlayer(name)
  return Player
end
"#,
    )
    .expect("Lua source with doc annotations should parse");
    let environment = build_type_environment(&parsed).expect("Lua doc annotations should lower");

    assert_eq!(
        environment.alias("PlayerId"),
        Some(&Type::union([Type::string(), Type::number()]))
    );
    assert_eq!(
        environment.alias_documentation("Player"),
        Some("player object")
    );
    assert_eq!(environment.local("Player"), Some(&Type::alias("Player")));
    assert_eq!(
        environment.local_documentation("Score"),
        Some("global score")
    );

    let Type::Table(player) = environment.alias("Player").expect("Player should exist") else {
        panic!("Player should lower to a table shape");
    };

    assert_eq!(
        player.field("hp").map(|field| &field.ty),
        Some(&Type::number())
    );
    assert_eq!(player.field("name").map(|field| field.optional), Some(true));

    let make_player = environment
        .function("makePlayer")
        .expect("doc function should lower");

    assert_eq!(make_player.parameters[0].name, "name");
    assert_eq!(make_player.parameters[0].ty, Type::string());
    assert_eq!(
        make_player.parameters[0].documentation.as_deref(),
        Some("player name")
    );
    assert_eq!(make_player.return_type, Some(Type::alias("Player")));
    assert_eq!(
        make_player.documentation.as_deref(),
        Some("player name\ncreated player")
    );
}

#[test]
fn lowers_lua_doc_string_literal_union_fields_as_literals() {
    let parsed = parse_lua_source(
        "src/enemy.lua",
        r#"---@class Enemy
---@field hp number
---@field type 'friendly' | 'mad'
local Enemy = {}
"#,
    )
    .expect("Lua source with doc annotations should parse");
    let environment = build_type_environment(&parsed).expect("Lua doc annotations should lower");

    let Type::Table(enemy) = environment.alias("Enemy").expect("Enemy should exist") else {
        panic!("Enemy should lower to a table shape");
    };

    assert_eq!(
        enemy.field("type").map(|field| &field.ty),
        Some(&Type::union([
            Type::string_literal("friendly"),
            Type::string_literal("mad"),
        ]))
    );
}

#[test]
fn lowers_lua_doc_overload_vararg_and_identifier_annotations() {
    let parsed = parse_lua_source(
        "src/load.lua",
        r#"---@param first identifier first path
---@vararg identifier extra paths
---@overload fun(path: identifier): Image single image
---@overload fun(path: identifier, kind: "stream"): Source streaming source
---@return Image primary image
local function load(first, ...)
  return nil
end
"#,
    )
    .expect("Lua source with overload annotations should parse");
    let environment =
        build_type_environment(&parsed).expect("LuaDoc overload annotations should lower");
    let load = environment
        .function("load")
        .expect("documented function should lower");

    assert_eq!(load.parameters[0].name, "first");
    assert_eq!(load.parameters[0].ty, Type::string());
    assert!(load.has_varargs);
    assert_eq!(load.vararg_type, Some(Type::string()));
    assert_eq!(load.return_types, vec![Type::alias("Image")]);
    assert_eq!(load.overloads.len(), 2);
    assert_eq!(load.overloads[0].parameters[0].ty, Type::string());
    assert_eq!(load.overloads[0].return_types, vec![Type::alias("Image")]);
    assert_eq!(
        load.overloads[0].documentation.as_deref(),
        Some("single image")
    );
    assert_eq!(
        load.overloads[1].parameters[1].ty,
        Type::string_literal("stream")
    );
    assert_eq!(load.overloads[1].return_types, vec![Type::alias("Source")]);
}

#[test]
fn native_and_lua_doc_function_types_share_semantic_representation() {
    let native = parse_tua_source(
        "src/native.tua",
        r#"type Handler = (value: number, ...string) -> boolean
"#,
    )
    .expect("native function alias should parse");
    let documented = parse_lua_source(
        "src/documented.lua",
        r#"---@alias Handler fun(value: number, ...: string): boolean
"#,
    )
    .expect("LuaDoc function alias should parse");
    let native_environment =
        build_type_environment(&native).expect("native function alias should lower");
    let documented_environment =
        build_type_environment(&documented).expect("LuaDoc function alias should lower");

    assert_eq!(
        native_environment.alias("Handler"),
        documented_environment.alias("Handler")
    );

    let Type::Function(handler) = native_environment
        .alias("Handler")
        .expect("native handler should exist")
    else {
        panic!("handler should lower to the shared function representation");
    };
    assert_eq!(handler.parameters[0].name.as_deref(), Some("value"));
    assert_eq!(handler.parameters[0].ty, Type::number());
    assert_eq!(handler.vararg_type.as_deref(), Some(&Type::string()));
    assert_eq!(handler.return_types_or_any(), &[Type::boolean()]);
}

#[test]
fn reports_conflicts_between_tua_and_lua_doc_annotations() {
    let parsed = parse_tua_source(
        "src/conflict.tua",
        r#"---@param value string documented type
local function test(value: number): number
  return value
end
"#,
    )
    .expect("source should parse");
    let diagnostics = build_type_environment(&parsed)
        .expect_err("conflicting doc and Tua annotations should be rejected");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_DOC_CONFLICT));
    assert!(diagnostics[0]
        .message
        .contains("conflicting annotations for `test` parameter `value`"));
}

#[test]
fn aliases_resolve_within_the_file_type_context() {
    let parsed = parse_tua_source(
        "src/aliases.tua",
        r#"
type HitPoints = number
type Health = HitPoints

local hp: Health = 10
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("aliases should resolve through context");
    let context = environment.to_type_context();

    assert!(context.is_assignable(&Type::number(), environment.local("hp").unwrap()));
    assert!(context.is_assignable(&Type::alias("Health"), &Type::alias("HitPoints")));
}

#[test]
fn aliases_can_refer_to_later_aliases_in_the_same_file() {
    let parsed = parse_tua_source(
        "src/forward.tua",
        r#"
type Player = {
  pos: Vec2,
}

type Vec2 = {
  x: number,
  y: number,
}
"#,
    )
    .expect("typed source should parse");
    let environment =
        build_type_environment(&parsed).expect("forward aliases should be known per file");

    let Type::Table(player) = environment
        .alias("Player")
        .expect("Player alias should exist")
    else {
        panic!("Player should be a table shape");
    };

    assert_eq!(player.field("pos").unwrap().ty, Type::alias("Vec2"));
}

#[test]
fn unknown_type_names_produce_type_diagnostics() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
local player: Player = nil
"#,
    )
    .expect("typed source should parse");
    let diagnostics =
        build_type_environment(&parsed).expect_err("unknown types should produce diagnostics");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_UNKNOWN_NAME));
    assert_eq!(
        diagnostics[0].code.unwrap().category(),
        crate::DiagnosticCategory::Type
    );
    assert!(diagnostics[0].message.contains("unknown type `Player`"));
    assert!(diagnostics[0].format_cli().contains("error TL1001"));
}

#[test]
fn unknown_type_names_inside_table_shapes_produce_type_diagnostics() {
    let parsed = parse_tua_source(
        "src/player.tua",
        r#"
type Player = {
  pos: MissingVec,
}
"#,
    )
    .expect("typed source should parse");
    let diagnostics =
        build_type_environment(&parsed).expect_err("unknown field types should be reported");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Some(DiagnosticCode::TYPE_UNKNOWN_NAME));
    assert!(diagnostics[0].message.contains("unknown type `MissingVec`"));
}

#[test]
fn duplicate_type_aliases_produce_deterministic_diagnostics() {
    let parsed = parse_tua_source(
        "src/types.tua",
        r#"
type Vec2 = {
  x: number,
}

type Player = {
  name: string,
}

type Vec2 = {
  y: number,
}

type Player = {
  hp: number,
}
"#,
    )
    .expect("typed source should parse");
    let diagnostics =
        build_type_environment(&parsed).expect_err("duplicate aliases should be rejected");

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        vec![
            "duplicate type alias `Vec2`",
            "duplicate type alias `Player`"
        ]
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == Some(DiagnosticCode::TYPE_DUPLICATE_ALIAS)));
    assert!(diagnostics[0].format_cli().contains("error TL1002"));
}

#[test]
fn recursive_luacats_aliases_preserve_outer_shape_and_truncate_the_back_edge() {
    let parsed = parse_lua_source(
        "types/feel.lua",
        r#"
---@class FeelAnimateStep
---@field kind "animate"
---@field duration number

---@class FeelParallelStep
---@field kind "parallel"
---@field steps FeelSequenceInput[]

---@alias FeelStep FeelAnimateStep|FeelParallelStep
---@alias FeelSequenceInput FeelStep|FeelStep[]
return {}
"#,
    )
    .expect("recursive LuaCATS declarations should parse");
    let environment =
        build_type_environment(&parsed).expect("recursive declarations should type-check");
    let resolved = environment.resolve_aliases(&Type::alias("FeelSequenceInput"));

    fn contains_any(ty: &Type) -> bool {
        match ty {
            Type::Any => true,
            Type::Union(types) | Type::Tuple(types) => types.iter().any(contains_any),
            Type::Function(function) => {
                function
                    .parameters
                    .iter()
                    .any(|parameter| contains_any(&parameter.ty))
                    || function.vararg_type.as_deref().is_some_and(contains_any)
                    || function.return_types_or_any().iter().any(contains_any)
            }
            Type::Table(table) => table.fields().any(|(_, field)| contains_any(&field.ty)),
            Type::Array(item) => contains_any(item),
            Type::Dictionary { key, value } => contains_any(key) || contains_any(value),
            Type::Primitive(_) | Type::Alias(_) | Type::StringLiteral(_) => false,
        }
    }

    fn contains_table(ty: &Type) -> bool {
        match ty {
            Type::Table(_) => true,
            Type::Union(types) | Type::Tuple(types) => types.iter().any(contains_table),
            Type::Array(item) => contains_table(item),
            Type::Function(_)
            | Type::Dictionary { .. }
            | Type::Primitive(_)
            | Type::Any
            | Type::Alias(_)
            | Type::StringLiteral(_) => false,
        }
    }

    let Type::Union(sequence) = &resolved else {
        panic!("expected the outer sequence union to remain available");
    };
    assert!(contains_table(&resolved));
    assert!(sequence
        .iter()
        .any(|member| matches!(member, Type::Array(_))));
    assert!(
        contains_any(&resolved),
        "the recursive edge should become any"
    );
}

#[test]
fn recursive_alias_back_edges_are_any_during_assignability() {
    let context = TypeContext::new().with_alias(TypeAlias::new(
        "Node",
        Type::table([
            ("next", TableField::required(Type::alias("Node"))),
            ("value", TableField::required(Type::number())),
        ]),
    ));
    let source = Type::table([
        ("next", TableField::required(Type::empty_table())),
        ("value", TableField::required(Type::number())),
    ]);
    let wrong = Type::table([
        ("next", TableField::required(Type::empty_table())),
        ("value", TableField::required(Type::string())),
    ]);

    assert!(context.is_assignable(&source, &Type::alias("Node")));
    assert!(!context.is_assignable(&wrong, &Type::alias("Node")));
}

fn vec2_type() -> Type {
    Type::table([
        ("x", TableField::required(Type::number())),
        ("y", TableField::required(Type::number())),
    ])
}

#[test]
fn infers_projected_coordinates_after_nil_guard() {
    let source = r#"
local function project(z: number)
  if z <= 0.05 then return nil end
  local width, height = love.graphics.getDimensions()
  local focal = (width / 2) / math.tan(math.rad(75) / 2)
  return width / 2 + focal / z, height / 2, focal / z, z
end
local function projected()
  local x, y = project(7)
  if not x then return nil end
  return { x = x, y = y }
end
"#;
    let parsed = parse_tua_source("src/projection.tua", source).unwrap();
    let environment = build_type_environment(&parsed).unwrap();
    let optional_number = Type::union([Type::nil(), Type::number()]);
    assert_eq!(
        environment.function("project").unwrap().return_types,
        vec![optional_number; 4]
    );
    let returns = environment
        .function("projected")
        .unwrap()
        .return_type
        .as_ref()
        .unwrap();
    assert_eq!(returns.to_string(), "nil | { x: number, y: number }");
}
