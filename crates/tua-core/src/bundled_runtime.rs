use std::collections::BTreeSet;

use full_moon::ast::{Call, Expression, FunctionArgs, FunctionCall, Prefix, Suffix};
use full_moon::tokenizer::TokenType;

use crate::module_link::{module_specifier_literals, ModuleSpecifierKind};

#[derive(Clone, Copy)]
pub struct BundledRuntime {
    pub module: &'static str,
    pub file_name: &'static str,
    pub source: fn() -> &'static str,
    pub dependencies: &'static [&'static str],
    pub explicit_import: bool,
}

pub const RUNTIMES: &[BundledRuntime] = &[
    BundledRuntime {
        module: "brinevector",
        file_name: "brinevector.lua",
        source: crate::brinevector_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "anim8",
        file_name: "anim8.lua",
        source: crate::anim8_runtime_source,
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuasprite",
        file_name: "tuasprite.lua",
        source: crate::tuasprite_runtime_source,
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuasignal",
        file_name: "tuasignal.lua",
        source: crate::tuasignal_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuaecs",
        file_name: "tuaecs.lua",
        source: crate::tuaecs_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuaobjectpool",
        file_name: "tuaobjectpool.lua",
        source: crate::tuaobjectpool_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuastatemachine",
        file_name: "tuastatemachine.lua",
        source: crate::tuastatemachine_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuastore",
        file_name: "tuastore.lua",
        source: crate::tuastore_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuaaudio",
        file_name: "tuaaudio.lua",
        source: crate::tuaaudio_runtime_source,
        dependencies: &["tuasignal", "tuastore"],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuatween",
        file_name: "tuatween.lua",
        source: crate::tuatween_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuacamera",
        file_name: "tuacamera.lua",
        source: crate::tuacamera_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "baton",
        file_name: "baton.lua",
        source: crate::baton_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
];

pub fn bundled_runtime(module: &str) -> Option<&'static BundledRuntime> {
    RUNTIMES.iter().find(|runtime| runtime.module == module)
}

pub fn bundled_runtime_requirements(source: &str) -> BTreeSet<&'static str> {
    let mut required = module_specifier_literals(source)
        .into_iter()
        .filter(|literal| literal.kind == ModuleSpecifierKind::Require && !literal.type_only)
        .filter_map(|literal| bundled_runtime(&literal.module).map(|runtime| runtime.module))
        .collect::<BTreeSet<_>>();
    let mut pending = required.iter().copied().collect::<Vec<_>>();

    while let Some(module) = pending.pop() {
        let Some(runtime) = bundled_runtime(module) else {
            continue;
        };
        for dependency in runtime.dependencies {
            if required.insert(dependency) {
                pending.push(dependency);
            }
        }
    }

    required
}

pub fn is_bundled_runtime_module(module: &str) -> bool {
    bundled_runtime(module).is_some()
}

pub(crate) fn required_module_from_call(call: &FunctionCall) -> Option<String> {
    let Prefix::Name(name) = call.prefix() else {
        return None;
    };
    if name.token().to_string().trim() != "require" {
        return None;
    }
    let Suffix::Call(Call::AnonymousCall(arguments)) = call.suffixes().next()? else {
        return None;
    };
    let token = match arguments {
        FunctionArgs::Parentheses { arguments, .. } => {
            let Expression::String(token) = arguments.iter().next()? else {
                return None;
            };
            token
        }
        FunctionArgs::String(token) => token,
        FunctionArgs::TableConstructor(_) => return None,
        _ => return None,
    };
    match token.token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirements_find_lua_imports_and_transitive_runtime_dependencies() {
        let required = bundled_runtime_requirements(
            r#"
local audio = require("tuaaudio")
local ignored = "require(\"brinevector\")"
-- require("tuaecs")
"#,
        );

        assert_eq!(
            required,
            BTreeSet::from(["tuaaudio", "tuasignal", "tuastore"])
        );
    }
}
