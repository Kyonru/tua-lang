#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::Once;
use tua_core::parse_config;

static INSTALL_RECOVERY_HOOK: Once = Once::new();

fuzz_target!(|data: &[u8]| {
    // Let Tua's configuration recovery boundary convert dependency panics into
    // normal parse errors before libFuzzer's default hook aborts the process.
    INSTALL_RECOVERY_HOOK.call_once(|| std::panic::set_hook(Box::new(|_| {})));

    let input = String::from_utf8_lossy(data);
    let _ = parse_config(&input);
});
