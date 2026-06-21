use super::*;

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
fn any_is_assignable_in_both_directions() {
    assert!(is_assignable(&Type::Any, &Type::number()));
    assert!(is_assignable(&Type::number(), &Type::Any));
    assert!(is_assignable(&Type::Any, &Type::empty_table()));
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
fn table_fields_must_be_assignable() {
    let source = Type::table([("hp", TableField::required(Type::string()))]);
    let target = Type::table([("hp", TableField::required(Type::number()))]);

    assert!(!is_assignable(&source, &target));
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
