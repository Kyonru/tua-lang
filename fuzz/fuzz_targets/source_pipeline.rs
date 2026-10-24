#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::Once;
use tua_core::{
    check_tua_source, emit_tua_source, format_tua_source, normalize_tua_source,
    parse_raw_lua_source, parse_raw_tua_source, FormatOptions,
};

static INSTALL_RECOVERY_HOOK: Once = Once::new();

fuzz_target!(|data: &[u8]| {
    // full_moon can panic internally for malformed syntax. Tua deliberately
    // catches that unwind and returns a diagnostic, but libFuzzer installs an
    // aborting hook that fires before the recovery boundary. An uncaught panic
    // still aborts when it reaches the C fuzz entry point, so it remains a
    // crash while recoverable parser panics can complete normally.
    INSTALL_RECOVERY_HOOK.call_once(|| std::panic::set_hook(Box::new(|_| {})));

    let source = String::from_utf8_lossy(data).into_owned();
    let normalized = normalize_tua_source(source.clone());
    let parsed = parse_raw_tua_source("fuzz/input.tua", source.clone());
    let _ = parse_raw_lua_source("fuzz/input.lua", source);

    if parsed.is_ok() {
        let _ = check_tua_source("fuzz/input.tua", normalized.original.clone());
        let _ = format_tua_source(
            "fuzz/input.tua",
            normalized.original.clone(),
            &FormatOptions::default(),
        );
        let _ = emit_tua_source("fuzz/input.tua", &normalized.original);
    }
});
