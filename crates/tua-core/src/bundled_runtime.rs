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
        module: "tuadialogue.history",
        file_name: "tuadialogue/history.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/history.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.conditions",
        file_name: "tuadialogue/conditions.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/conditions.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuasave_shape",
        file_name: "tuasave_shape.lua",
        source: || include_str!("../../../vendor/runtime/tuasave_shape.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.text",
        file_name: "tuadialogue/text.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/text.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.checkpoint",
        file_name: "tuadialogue/checkpoint.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/checkpoint.lua"),
        dependencies: &["tuasave_json"],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.media",
        file_name: "tuadialogue/media.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/media.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.actions",
        file_name: "tuadialogue/actions.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/actions.lua"),
        dependencies: &["tuasave_shape"],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue/unicode.LICENSE",
        file_name: "tuadialogue/unicode.LICENSE",
        source: || include_str!("../../../vendor/runtime/tuadialogue/unicode.LICENSE"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.unicode_data",
        file_name: "tuadialogue/unicode_data.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/unicode_data.lua"),
        dependencies: &["tuadialogue/unicode.LICENSE"],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.graphemes",
        file_name: "tuadialogue/graphemes.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/graphemes.lua"),
        dependencies: &["tuadialogue.unicode_data"],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.rich",
        file_name: "tuadialogue/rich.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/rich.lua"),
        dependencies: &["tuadialogue.graphemes", "tuadialogue.text"],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.timing",
        file_name: "tuadialogue/timing.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/timing.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue.reveal",
        file_name: "tuadialogue/reveal.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue/reveal.lua"),
        dependencies: &["tuadialogue.graphemes", "tuadialogue.timing"],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuadialogue",
        file_name: "tuadialogue.lua",
        source: || include_str!("../../../vendor/runtime/tuadialogue.lua"),
        dependencies: &[
            "tuadialogue.text",
            "tuasave_shape",
            "tuadialogue.conditions",
            "tuadialogue.actions",
            "tuadialogue.media",
            "tuadialogue.checkpoint",
            "tuadialogue.history",
            "tuadialogue.reveal",
            "tuadialogue.rich",
        ],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuaboundedqueue",
        file_name: "tuaboundedqueue.lua",
        source: || include_str!("../../../vendor/runtime/tuaboundedqueue.lua"),
        dependencies: &[],
        explicit_import: true,
    },
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
        module: "tuai18n",
        file_name: "tuai18n.lua",
        source: crate::tuai18n_runtime_source,
        dependencies: &[
            "tuadialogue.text",
            "smiti18n",
            "smiti18n.format",
            "smiti18n.plural",
            "smiti18n.interpolate",
            "smiti18n.variants",
            "smiti18n.version",
            "smiti18n.LICENSE",
        ],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuasave",
        file_name: "tuasave.lua",
        source: crate::tuasave_runtime_source,
        dependencies: &["tuasave_json", "tuasave_shape"],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tuasave_json",
        file_name: "tuasave_json.lua",
        source: crate::tuasave_json_runtime_source,
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "smiti18n",
        file_name: "smiti18n/init.lua",
        source: || include_str!("../../../vendor/runtime/smiti18n/init.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "smiti18n.format",
        file_name: "smiti18n/format.lua",
        source: || include_str!("../../../vendor/runtime/smiti18n/format.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "smiti18n.plural",
        file_name: "smiti18n/plural.lua",
        source: || include_str!("../../../vendor/runtime/smiti18n/plural.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "smiti18n.interpolate",
        file_name: "smiti18n/interpolate.lua",
        source: || include_str!("../../../vendor/runtime/smiti18n/interpolate.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "smiti18n.variants",
        file_name: "smiti18n/variants.lua",
        source: || include_str!("../../../vendor/runtime/smiti18n/variants.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "smiti18n.version",
        file_name: "smiti18n/version.lua",
        source: || include_str!("../../../vendor/runtime/smiti18n/version.lua"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "smiti18n.LICENSE",
        file_name: "smiti18n/LICENSE",
        source: || include_str!("../../../vendor/runtime/smiti18n/LICENSE"),
        dependencies: &[],
        explicit_import: false,
    },
    BundledRuntime {
        module: "tua.audio.design",
        file_name: "tua/audio/design.lua",
        source: crate::tua_audio_design_runtime_source,
        dependencies: &[],
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
        module: "tuashaderscan",
        file_name: "tuashaderscan.lua",
        source: crate::tuashaderscan_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuarot",
        file_name: "tuarot.lua",
        source: crate::tuarot_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tuabehavior",
        file_name: "tuabehavior.lua",
        source: crate::tuabehavior_runtime_source,
        dependencies: &[],
        explicit_import: true,
    },
    BundledRuntime {
        module: "tualog",
        file_name: "tualog.lua",
        source: crate::tualog_runtime_source,
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

    #[test]
    fn procedural_audio_runtime_is_included_only_when_imported() {
        assert!(bundled_runtime_requirements("return 1").is_empty());
        assert_eq!(
            bundled_runtime_requirements("local design = require('tua.audio.design')\n"),
            BTreeSet::from(["tua.audio.design"])
        );
    }
}
