//! Tua's dialect must not change the ordinary Lua/Luau parsing contract.
#![cfg(feature = "luau")]
use full_moon::{parse_fallible, LuaVersion};

#[test]
fn authored_extensions_require_the_tua_dialect() {
    for source in [
        "local values: number[] = {1}\n",
        "local function f(value: number = 1) return value end\n",
        "local function f(value?: number) return value end\n",
        "type Value = { name?: string }\n",
        "class Actor { hp: number = 1 }\n",
        "protocol Actor { draw(): nil }\n",
        "enum Mode \"idle\" \"active\" end\n",
        "import type { Actor as Player } from \"actors\"\n",
        "local matches = actor is Actor\n",
    ] {
        let ast = parse_fallible(source, LuaVersion::tua())
            .into_result()
            .unwrap();
        assert_eq!(ast.to_string(), source);
        assert!(
            parse_fallible(source, LuaVersion::luau())
                .into_result()
                .is_err(),
            "{source}"
        );
    }
}

#[test]
fn contextual_words_remain_ordinary_identifiers() {
    let source = "local is, class, protocol, enum, import = 1, 2, 3, 4, 5\nreturn is + class + protocol + enum + import\n";
    for version in [LuaVersion::lua51(), LuaVersion::luau(), LuaVersion::tua()] {
        assert_eq!(
            parse_fallible(source, version)
                .into_result()
                .unwrap()
                .to_string(),
            source
        );
    }
}
