#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::{Component, Path};

fuzz_target!(|data: &[u8]| {
    tua_lsp::fuzz_transport(data);
    let root = Path::new("/tua-fuzz-root");
    let name = String::from_utf8_lossy(data);
    let target = root.join(name.as_ref());
    if tua_core::path_security::reject_symlink_path(root, &target).is_ok() {
        let relative = target.strip_prefix(root).unwrap();
        assert!(relative
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir)));
    }
});
