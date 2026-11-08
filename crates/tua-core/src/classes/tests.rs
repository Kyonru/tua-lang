use std::path::Path;

use crate::parser::{GenericDeclarationKind, TuaType};

use super::parse_class_declarations;

#[test]
fn generic_class_headers_preserve_declaration_identity_and_parent_arguments() {
    let source =
        "open class Base<T> {\n  value: T\n}\nfinal class Child<U> extends Base<Box<U[]>> {\n}\n";
    let path = Path::new("src/classes.tua");
    let (classes, diagnostics) = parse_class_declarations(path, source);

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let base = &classes[0];
    let child = &classes[1];
    assert_eq!(base.generic_parameters.len(), 1);
    assert_eq!(base.generic_parameters[0].name, "T");
    assert_eq!(base.generic_parameters[0].name_span.start.line, 1);
    assert_eq!(base.generic_parameters[0].name_span.start.column, 17);
    assert_eq!(
        base.generic_parameters[0].id.declaration.kind,
        GenericDeclarationKind::Class
    );
    assert_eq!(
        base.generic_parameters[0].id.declaration.module,
        crate::vfs::normalize_path(path)
    );

    assert_eq!(child.generic_parameters.len(), 1);
    assert_eq!(child.generic_parameters[0].name, "U");
    assert_ne!(
        base.generic_parameters[0].id, child.generic_parameters[0].id,
        "same-looking class binders must have declaration-owned identities"
    );
    assert_eq!(child.parent.as_deref(), Some("Base"));
    let application = child
        .parent_application
        .as_ref()
        .expect("extends syntax should retain the applied parent for G3");
    assert_eq!(application.name, "Base");
    assert_eq!(application.arguments.len(), 1);
    assert!(matches!(
        &application.arguments[0].ty,
        TuaType::Applied { name, arguments, .. }
            if name == "Box"
                && matches!(
                    arguments.as_slice(),
                    [TuaType::Array(inner)]
                        if matches!(inner.as_ref(), TuaType::Parameter { id, name } if id == &child.generic_parameters[0].id && name == "U")
                )
    ));
    assert!(application.span.end.column > application.name_span.end.column);
}

#[test]
fn bare_parent_headers_keep_the_legacy_parent_and_record_empty_application() {
    let source = "open class Base {\n}\nfinal class Child extends Base {\n}\n";
    let (classes, diagnostics) = parse_class_declarations(Path::new("src/child.tua"), source);

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(classes[1].parent.as_deref(), Some("Base"));
    assert_eq!(
        classes[1]
            .parent_application
            .as_ref()
            .map(|application| application.arguments.len()),
        Some(0)
    );
    assert!(classes[1]
        .parent_application
        .as_ref()
        .is_some_and(|application| application.arguments.is_empty()));
}

#[test]
fn generic_class_headers_and_parent_arguments_are_erased_from_lua() {
    let source =
        "open class Base<T> {\n  value: T\n}\nfinal class Child<U> extends Base<Box<U[]>> {\n}\n";
    let parsed = crate::parser::parse_tua_source("src/classes.tua", source)
        .expect("generic class syntax should normalize for existing class lowering");
    let lua = crate::emit::emit_lua(&parsed).expect("class type syntax is erased before emission");

    assert!(!parsed.raw.parser_source.contains("Base<T>"));
    assert!(!parsed.raw.parser_source.contains("Base<Box<U[]>>"));
    assert!(!lua.contains("class Base"));
    assert!(!lua.contains("extends"));
    assert!(!lua.contains("Box<U"));
    assert!(lua.contains("setmetatable(Child, { __index = Base })"));
}
