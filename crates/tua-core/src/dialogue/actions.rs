//! Action factories retain concrete handler arguments across shared inference paths.
use super::*;

pub(super) fn effect_type() -> Type {
    Type::union([
        Type::string_literal("presentation"),
        Type::string_literal("consequential"),
    ])
}

pub(super) fn spec_type() -> Type {
    Type::table([
        (
            "payload",
            TableField::required(crate::save::shape_type(Type::Any)),
        ),
        ("effect", TableField::required(effect_type())),
    ])
}

fn fields(payloads: &Type) -> Option<&crate::types::TableShape> {
    match payloads {
        Type::Readonly(inner) => fields(inner),
        Type::Table(table) => Some(table),
        _ => None,
    }
}

pub(super) fn options(options: &Type, payloads: &Type, player: &Type) -> Type {
    if let Type::Union(members) = options {
        return Type::union(
            members
                .iter()
                .map(|member| self::options(member, payloads, player)),
        );
    }
    let (Type::Table(mut options), Type::Table(player), Some(payloads)) =
        (options.clone(), player, fields(payloads))
    else {
        return options.clone();
    };
    let Type::Function(snapshot) = &player.field("snapshot").expect("snapshot contract").ty else {
        unreachable!()
    };
    let Type::Table(snapshot) = snapshot.return_type.as_ref() else {
        unreachable!()
    };
    let request = snapshot
        .field("action")
        .expect("action request contract")
        .ty
        .clone();
    let handlers = Type::table(payloads.fields().map(|(name, field)| {
        (
            name.to_owned(),
            TableField::optional(signature(
                vec![("payload", field.ty.clone()), ("request", request.clone())],
                2,
                Type::boolean(),
            )),
        )
    }));
    options.insert("handlers", TableField::optional(handlers));
    Type::Table(options)
}

pub(super) fn definition(
    payloads: Type,
    document: &Type,
    base_options: &Type,
    player: &Type,
) -> Type {
    let effects = Type::table(
        fields(&payloads)
            .expect("payload fields")
            .fields()
            .map(|(name, _)| (name.to_owned(), TableField::required(effect_type()))),
    );
    Type::table([
        (
            "kind",
            TableField::required(Type::string_literal("actions")),
        ),
        (
            "payloads",
            TableField::required(Type::Readonly(Box::new(payloads.clone()))),
        ),
        (
            "effects",
            TableField::required(Type::Readonly(Box::new(effects))),
        ),
        (
            "new",
            TableField::required(signature(
                vec![
                    ("source", document.clone()),
                    ("options", options(base_options, &payloads, player)),
                ],
                1,
                player.clone(),
            )),
        ),
    ])
}
