use super::*;
use std::collections::BTreeMap;

#[test]
fn remaps_direct_lua_traceback_locations() {
    let maps = vec![TracebackSourceMap {
        generated: "build/main.lua".to_owned(),
        source: "src/main.tua".to_owned(),
        lines: BTreeMap::from([(4, 9)]),
    }];

    let remapped = remap_traceback_text(
        "build/main.lua:4: attempt to index nil\nstack traceback:\n",
        &maps,
    );

    assert_eq!(
        remapped,
        "src/main.tua:9 (generated build/main.lua:4): attempt to index nil\nstack traceback:\n"
    );
}

#[test]
fn leaves_ambiguous_basename_only_tracebacks_unchanged() {
    let maps = vec![
        TracebackSourceMap {
            generated: "build/client/main.lua".to_owned(),
            source: "src/client/main.tua".to_owned(),
            lines: BTreeMap::from([(4, 9)]),
        },
        TracebackSourceMap {
            generated: "build/server/main.lua".to_owned(),
            source: "src/server/main.tua".to_owned(),
            lines: BTreeMap::from([(4, 9)]),
        },
    ];

    assert_eq!(
        remap_traceback_text("main.lua:4: boom\n", &maps),
        "main.lua:4: boom\n"
    );
}
