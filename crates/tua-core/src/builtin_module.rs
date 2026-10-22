//! Built-in runtime modules typed from their own LuaDoc.
//!
//! `tween` and `Camera` are globals in Tua source, like `ecs`: using one emits
//! `local tween = require("tuatween")`, and `tua build` copies the runtime.
//! Their types come from the runtime's annotations, so the Lua file is the
//! single source of truth for behavior and checking.

use std::cell::Cell;
use std::path::Path;
use std::sync::OnceLock;

use std::collections::BTreeSet;

use full_moon::ast;
use full_moon::visitors::Visitor;

use crate::types::{build_type_environment, Type, TypeEnvironment};
use crate::ParsedSource;

pub(crate) struct BuiltinModule {
    /// The global name used in Tua source.
    pub name: &'static str,
    /// The `require` name of the bundled runtime.
    pub runtime: &'static str,
    pub source: &'static str,
    pub summary: &'static str,
    pub documentation: &'static str,
}

pub(crate) const TWEEN: BuiltinModule = BuiltinModule {
    name: "tween",
    runtime: "tuatween",
    source: include_str!("../../../vendor/runtime/tuatween.lua"),
    summary: "built-in tweening module",
    documentation: "Tween numeric fields of any table with flux: \
        `tween.to(target, seconds, values)`, then call `tween.update(dt)` every frame.",
};

pub(crate) const CAMERA: BuiltinModule = BuiltinModule {
    name: "Camera",
    runtime: "tuacamera",
    source: include_str!("../../../vendor/runtime/tuacamera.lua"),
    summary: "built-in 2D camera module",
    documentation: "Create a 2D camera with `Camera.new({...})`: follow styles, \
        smoothing, bounds, zoom, shake, viewports, and screen/world conversion.",
};

pub(crate) const MODULES: &[BuiltinModule] = &[TWEEN, CAMERA];

pub(crate) fn module(name: &str) -> Option<&'static BuiltinModule> {
    MODULES.iter().find(|module| module.name == name)
}

/// The module's export type, inferred once from its LuaDoc with the runtime's
/// own classes inlined, so no internal type names leak into user code.
pub(crate) fn module_type(module: &BuiltinModule) -> Type {
    static TYPES: OnceLock<Vec<Type>> = OnceLock::new();
    let types = TYPES.get_or_init(|| {
        // Inference recurses deeply over a whole module. Run it once on a
        // thread with a known stack, whichever thread asks first.
        std::thread::Builder::new()
            .name("tua-builtin-modules".to_owned())
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                // The runtimes' own environments must not include the modules
                // being inferred, or inference would wait on itself.
                INFERRING.with(|inferring| inferring.set(true));
                MODULES.iter().map(infer_module_type).collect()
            })
            .and_then(|thread| {
                thread
                    .join()
                    .map_err(|_| std::io::Error::other("built-in module inference panicked"))
            })
            .unwrap_or_else(|_| vec![Type::Any; MODULES.len()])
    });
    let index = MODULES
        .iter()
        .position(|candidate| candidate.name == module.name)
        .expect("module comes from MODULES");
    types[index].clone()
}

fn infer_module_type(module: &BuiltinModule) -> Type {
    let path = Path::new(module.runtime).with_extension("lua");
    let Ok(parsed) = crate::parse_lua_source(&path, module.source.to_owned()) else {
        return Type::Any;
    };
    let Ok(environment) = build_type_environment(&parsed) else {
        return Type::Any;
    };
    crate::module_index::infer_module_exports_with_optional_environment(&parsed, Some(&environment))
        .map(|exports| inline_aliases(&exports.value_type, &environment, &mut Vec::new()))
        .unwrap_or(Type::Any)
}

/// Replace aliases defined by the runtime with their structure. A class that
/// refers to itself becomes `any` at the back-edge, as it does across module
/// boundaries in a project.
fn inline_aliases(ty: &Type, environment: &TypeEnvironment, visiting: &mut Vec<String>) -> Type {
    match ty {
        Type::Alias(name) => {
            if visiting.contains(name) {
                return Type::Any;
            }
            let Some(definition) = environment.alias(name) else {
                return ty.clone();
            };
            visiting.push(name.clone());
            let inlined = inline_aliases(definition, environment, visiting);
            visiting.pop();
            inlined
        }
        Type::Union(types) => Type::union(
            types
                .iter()
                .map(|ty| inline_aliases(ty, environment, visiting)),
        ),
        Type::Function(function) => {
            let mut function = function.clone();
            for parameter in &mut function.parameters {
                parameter.ty = inline_aliases(&parameter.ty, environment, visiting);
            }
            if let Some(vararg) = function.vararg_type.as_mut() {
                **vararg = inline_aliases(vararg, environment, visiting);
            }
            *function.return_type = inline_aliases(&function.return_type, environment, visiting);
            for return_type in &mut function.return_types {
                *return_type = inline_aliases(return_type, environment, visiting);
            }
            Type::Function(function)
        }
        Type::Table(table) => {
            let mut inlined = table.clone();
            let fields = table
                .fields()
                .map(|(name, field)| (name.to_owned(), field.clone()))
                .collect::<Vec<_>>();
            for (name, mut field) in fields {
                field.ty = inline_aliases(&field.ty, environment, visiting);
                inlined.insert(name, field);
            }
            Type::Table(inlined)
        }
        Type::Array(item) => Type::array(inline_aliases(item, environment, visiting)),
        Type::Tuple(items) => Type::tuple(
            items
                .iter()
                .map(|item| inline_aliases(item, environment, visiting)),
        ),
        Type::Dictionary { key, value } => Type::dictionary(
            inline_aliases(key, environment, visiting),
            inline_aliases(value, environment, visiting),
        ),
        Type::Primitive(_) | Type::Any | Type::StringLiteral(_) => ty.clone(),
    }
}

thread_local! {
    static INFERRING: Cell<bool> = const { Cell::new(false) };
}

/// Locals that make the built-in modules visible in `parsed`. A module whose
/// name the file declares anywhere, as a local, parameter, function, or
/// global assignment, is left out so the project's own value wins.
pub(crate) fn builtin_locals(parsed: &ParsedSource) -> Vec<(String, Type)> {
    if INFERRING.with(Cell::get) {
        return Vec::new();
    }
    let declared = declared_names(parsed);
    MODULES
        .iter()
        .filter(|module| !declared.contains(module.name))
        .map(|module| (module.name.to_owned(), module_type(module)))
        .collect()
}

/// Whether `name` refers to the built-in module in `parsed`.
pub(crate) fn is_visible(parsed: &ParsedSource, name: &str) -> bool {
    module(name).is_some() && !declared_names(parsed).contains(name)
}

fn declared_names(parsed: &ParsedSource) -> BTreeSet<String> {
    #[derive(Default)]
    struct Declarations(BTreeSet<String>);

    impl Visitor for Declarations {
        fn visit_local_assignment(&mut self, local: &ast::LocalAssignment) {
            self.0
                .extend(local.names().iter().map(|name| name.token().to_string()));
        }

        fn visit_local_function(&mut self, function: &ast::LocalFunction) {
            self.0.insert(function.name().token().to_string());
        }

        fn visit_function_declaration(&mut self, function: &ast::FunctionDeclaration) {
            let name = function.name();
            if name.names().len() == 1 && name.method_name().is_none() {
                if let Some(name) = name.names().iter().next() {
                    self.0.insert(name.token().to_string());
                }
            }
        }

        fn visit_function_body(&mut self, body: &ast::FunctionBody) {
            for parameter in body.parameters() {
                if let ast::Parameter::Name(name) = parameter {
                    self.0.insert(name.token().to_string());
                }
            }
        }

        fn visit_assignment(&mut self, assignment: &ast::Assignment) {
            for variable in assignment.variables() {
                if let ast::Var::Name(name) = variable {
                    self.0.insert(name.token().to_string());
                }
            }
        }
    }

    let mut declarations = Declarations::default();
    declarations.visit_ast(&parsed.ast);
    declarations.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_types_are_structural_and_carry_their_contracts() {
        for module in MODULES {
            let ty = module_type(module);
            let Type::Table(_) = &ty else {
                panic!("{} should export a table, found {ty}", module.name);
            };
            let rendered = ty.to_string();
            for internal in [
                "Tween",
                "TweenGroup",
                "Camera",
                "CameraOptions",
                "CameraRect",
            ] {
                assert!(
                    !rendered.contains(&format!(": {internal}"))
                        && !rendered.contains(&format!("-> {internal}")),
                    "{}: {rendered}",
                    module.name
                );
            }
        }

        let tween = module_type(module("tween").expect("tween"));
        assert!(tween.to_string().contains("\"quadout\""), "{tween}");
        let camera = module_type(module("Camera").expect("Camera"));
        assert!(camera.to_string().contains("\"platformer\""), "{camera}");
    }

    #[test]
    fn using_a_module_emits_its_require_once() {
        let emitted = crate::emit_tua_source(
            "src/main.tua",
            "local box = { x = 0 }\ntween.to(box, 1, { x = 1 })\ntween.update(0.1)\nlocal cam = Camera.new()\n",
        )
        .expect("emit");

        assert_eq!(
            emitted.matches("require(\"tuatween\")").count(),
            1,
            "{emitted}"
        );
        assert_eq!(
            emitted.matches("require(\"tuacamera\")").count(),
            1,
            "{emitted}"
        );
        assert!(
            emitted.starts_with("local tween = require(\"tuatween\")\n"),
            "{emitted}"
        );
    }

    #[test]
    fn a_project_binding_or_member_access_is_not_the_builtin() {
        let emitted = crate::emit_tua_source(
            "src/main.tua",
            "local tween = { to = function() end }\ntween.to()\nlocal ui = { Camera = { new = function() end } }\nui.Camera.new()\n",
        )
        .expect("emit");

        assert!(!emitted.contains("require("), "{emitted}");
        let checked = crate::check_tua_source(
            "src/main.tua",
            "local tween = { to = function(n: number): number return n end }\nlocal value: number = tween.to(1)\nprint(value)\n",
        );
        assert!(checked.is_ok(), "{checked:?}");
    }

    #[test]
    fn module_calls_are_checked_against_the_runtime_annotations() {
        let diagnostics = crate::check_tua_source(
            "src/main.tua",
            r#"local cam = Camera.new({ style = "orbit", zoom = 2 })
cam:shake("hard")
tween.to({ x = 0 }, 1, { x = "far" })
tween.update(0.016)
"#,
        )
        .expect_err("misuse should be reported");
        let messages = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.message.clone()))
            .collect::<Vec<_>>();

        assert!(
            messages
                .iter()
                .any(|(line, message)| *line == 1 && message.contains("found \"orbit\"")),
            "{messages:#?}"
        );
        assert!(
            messages
                .iter()
                .any(|(line, message)| *line == 2 && message.contains("trauma")),
            "{messages:#?}"
        );
        assert!(
            messages
                .iter()
                .any(|(line, message)| *line == 3 && message.contains("values.x")),
            "{messages:#?}"
        );
        assert!(
            !messages.iter().any(|(line, _)| *line == 4),
            "{messages:#?}"
        );
    }

    #[test]
    fn modules_have_hover_and_completion() {
        let hover = crate::hover_tua_source(
            "src/main.tua",
            "local cam = Camera.new()\nprint(cam)\n",
            crate::SourcePosition {
                line: 1,
                column: 14,
            },
        )
        .expect("hover")
        .expect("a hover for Camera");
        assert!(
            hover.contents.contains("(built-in module) Camera"),
            "{}",
            hover.contents
        );

        let completions = crate::complete_tua_source(
            "src/main.tua",
            "local cam = Camera.new()\ntw",
            crate::SourcePosition { line: 2, column: 3 },
        )
        .expect("completions");
        assert!(
            completions
                .iter()
                .any(|completion| completion.label == "tween"),
            "{:?}",
            completions
                .iter()
                .map(|completion| &completion.label)
                .collect::<Vec<_>>()
        );
    }
}
