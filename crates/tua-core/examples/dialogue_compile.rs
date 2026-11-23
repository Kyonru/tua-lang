//! Developer harness for the shared dialogue frontend, before project integration.
use std::path::PathBuf;

fn main() {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: dialogue_compile <source.tdialogue>");
        std::process::exit(2);
    };
    let source = tua_core::input_limits::read_source(&path).unwrap_or_else(|error| {
        eprintln!("{}: {error}", path.display());
        std::process::exit(1);
    });
    match tua_core::emit::emit_dialogue_source(&path, &source) {
        Ok(emission) => print!("{}", emission.code),
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{}", diagnostic.format_cli());
            }
            std::process::exit(1);
        }
    }
}
