//! `tua dev`: watch, rebuild, and hot reload a running LOVE game.
//!
//! After each error-free build, the output's main.lua ends with
//! `require("tuahotreload").install()` and `.tuahotreload` is rewritten. The
//! runtime in the game watches that file and swaps changed modules in place.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use tua_core::{tuahotreload_runtime_source, LoadedConfig, ProjectWorkspace};

use crate::build::{has_errors, BuildOutcome, OutputGuard};
use crate::error::CliError;
use crate::love::{love_commands, LOVE_NOT_FOUND};
use crate::watch::run_watch_with_hook;

pub(crate) const RUNTIME_FILE: &str = "tuahotreload.lua";
const SIGNAL_FILE: &str = ".tuahotreload";
const HOOK: &str = "require(\"tuahotreload\").install()";

pub(crate) fn run_dev(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    love: Option<PathBuf>,
    no_launch: bool,
) -> Result<(), CliError> {
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
    let out_dir = workspace.primary_output_root().to_path_buf();
    let output_guard = OutputGuard::new(&workspace).map_err(|diagnostic| {
        CliError::from_diagnostics("unsafe dev output path", vec![diagnostic])
    })?;
    let mut generation = 0u64;
    let mut launched = false;

    run_watch_with_hook(cwd, loaded, &mut |outcome: &BuildOutcome| {
        if has_errors(&outcome.diagnostics) {
            println!(
                "{}",
                if launched {
                    "dev: keeping the running game on its last good build"
                } else {
                    "dev: fix the errors above to start the game"
                }
            );
            return Ok(());
        }
        prepare_output(&output_guard, &out_dir).map_err(|error| io_error(&out_dir, error))?;
        generation += 1;
        write_signal(
            &output_guard,
            &out_dir,
            generation,
            &outcome.shader_dependencies,
        )
        .map_err(|error| io_error(&out_dir, error))?;
        if !launched {
            if no_launch {
                // Emitted only after all output and the initial reload signal are ready.
                println!("dev: ready {}", out_dir.display());
            } else {
                let child = launch(love.as_deref(), &out_dir)?;
                exit_with_game(child);
            }
            launched = true;
        } else {
            println!("dev: sent reload {generation} to the game");
        }
        Ok(())
    })
}

/// Make the build output hot reloadable: the runtime beside it and the
/// install hook at the end of main.lua, where it does not shift source-map
/// lines.
fn prepare_output(guard: &OutputGuard, out_dir: &Path) -> io::Result<()> {
    let runtime = out_dir.join(RUNTIME_FILE);
    check_output_path(guard, &runtime)?;
    if fs::read_to_string(&runtime).ok().as_deref() != Some(tuahotreload_runtime_source()) {
        fs::write(&runtime, tuahotreload_runtime_source())?;
    }

    let main = out_dir.join("main.lua");
    check_output_path(guard, &main)?;
    let source = fs::read_to_string(&main).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "{} must exist for LOVE to start the game: {error}",
                main.display()
            ),
        )
    })?;
    if !source.lines().any(|line| line.trim() == HOOK) {
        let separator = if source.ends_with('\n') { "" } else { "\n" };
        fs::write(&main, format!("{source}{separator}{HOOK}\n"))?;
    }
    Ok(())
}

/// Written last, so the game reloads only finished output. Unix can replace
/// the signal atomically; Windows requires removing the previous tiny signal
/// file before renaming its replacement.
fn write_signal(
    guard: &OutputGuard,
    out_dir: &Path,
    generation: u64,
    shaders: &[crate::build::ShaderDependency],
) -> io::Result<()> {
    let temporary = out_dir.join(format!("{SIGNAL_FILE}.tmp"));
    let signal = out_dir.join(SIGNAL_FILE);
    check_output_path(guard, &temporary)?;
    check_output_path(guard, &signal)?;
    let mut contents = format!("{generation}\n");
    // Include the complete map so games that miss a generation still detect
    // changed shader content. Hex keeps arbitrary filenames inert and unambiguous.
    for dependency in shaders {
        let hex = |value: &str| {
            value
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        contents.push_str(&format!(
            "shader\t{}\t{}\n",
            hex(&dependency.shader),
            hex(&dependency.module)
        ));
    }
    fs::write(&temporary, contents)?;
    replace_signal(&temporary, &signal)
}

fn check_output_path(guard: &OutputGuard, path: &Path) -> io::Result<()> {
    guard
        .check(path)
        .map_err(|diagnostic| io::Error::new(io::ErrorKind::InvalidInput, diagnostic.message))
}

#[cfg(not(windows))]
fn replace_signal(temporary: &Path, signal: &Path) -> io::Result<()> {
    fs::rename(temporary, signal)
}

#[cfg(windows)]
fn replace_signal(temporary: &Path, signal: &Path) -> io::Result<()> {
    match fs::remove_file(signal) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::rename(temporary, signal)
}

fn launch(love: Option<&Path>, out_dir: &Path) -> Result<Child, CliError> {
    for command in &love_commands(love) {
        match Command::new(command).arg(out_dir).spawn() {
            Ok(child) => {
                println!("dev: started {} {}", command.display(), out_dir.display());
                return Ok(child);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(CliError::usage(format!(
                    "cannot start {}: {error}",
                    command.display()
                )))
            }
        }
    }
    Err(CliError::usage(LOVE_NOT_FOUND))
}

/// End `tua dev` with the game's exit status when the window closes. The
/// watch loop holds stdout and stderr for its whole run, so this thread exits
/// without printing.
fn exit_with_game(mut child: Child) {
    std::thread::spawn(move || {
        let code = child
            .wait()
            .ok()
            .and_then(|status| status.code())
            .unwrap_or(1);
        std::process::exit(code);
    });
}

fn io_error(path: &Path, error: io::Error) -> CliError {
    CliError::usage(format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tua-dev-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn output_gets_the_runtime_and_one_install_hook() {
        let out = temp_dir("prepare");
        let guard = OutputGuard::for_root(&out);
        fs::write(out.join("main.lua"), "print('hi')").unwrap();

        prepare_output(&guard, &out).unwrap();
        prepare_output(&guard, &out).unwrap();

        let main = fs::read_to_string(out.join("main.lua")).unwrap();
        assert_eq!(main, format!("print('hi')\n{HOOK}\n"));
        assert_eq!(
            fs::read_to_string(out.join(RUNTIME_FILE)).unwrap(),
            tuahotreload_runtime_source()
        );
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn signals_replace_the_previous_generation() {
        let out = temp_dir("signal");
        let guard = OutputGuard::for_root(&out);
        write_signal(&guard, &out, 1, &[]).unwrap();
        write_signal(&guard, &out, 2, &[]).unwrap();

        assert_eq!(fs::read_to_string(out.join(SIGNAL_FILE)).unwrap(), "2\n");
        assert!(!out.join(format!("{SIGNAL_FILE}.tmp")).exists());
        fs::remove_dir_all(out).unwrap();
    }

    #[test]
    fn shader_only_rebuild_signals_unchanged_loading_modules() {
        let root = temp_dir("shader-dependencies");
        fs::create_dir_all(root.join("src/game")).unwrap();
        fs::write(root.join("tua.toml"), "source = \"src\"\n").unwrap();
        fs::write(
            root.join("src/main.tua"),
            "local water = require(\"game.water\")\nreturn water\n",
        )
        .unwrap();
        fs::write(
            root.join("src/game/water.tua"),
            "local water = shader \"water.glsl\"\nreturn { shader = water }\n",
        )
        .unwrap();
        let shader = root.join("src/water.glsl");
        fs::write(
            &shader,
            "vec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return c; }",
        )
        .unwrap();
        let loaded = tua_core::load_config_from_dir(&root).unwrap();
        let (mut session, first) = crate::build::WatchBuildSession::start(&root, loaded).unwrap();
        assert!(!has_errors(&first.diagnostics), "{:?}", first.diagnostics);
        assert_eq!(
            first.shader_dependencies,
            vec![crate::build::ShaderDependency {
                shader: "water.glsl".into(),
                module: "game/water.lua".into(),
            }]
        );
        let lua = fs::read(root.join("build/game/water.lua")).unwrap();
        fs::write(
            &shader,
            "vec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return c * 0.5; }",
        )
        .unwrap();
        let next = session.rebuild(vec![shader], vec![]);
        assert!(!has_errors(&next.diagnostics), "{:?}", next.diagnostics);
        assert_eq!(next.shader_dependencies, first.shader_dependencies);
        assert_eq!(fs::read(root.join("build/game/water.lua")).unwrap(), lua);
        let guard = OutputGuard::for_root(&root);
        write_signal(&guard, &root.join("build"), 2, &next.shader_dependencies).unwrap();
        let signal = fs::read_to_string(root.join("build/.tuahotreload")).unwrap();
        assert_eq!(
            signal,
            "2\nshader\t77617465722e676c736c\t67616d652f77617465722e6c7561\n"
        );
        // A subsequent non-shader rebuild retains the map for games that missed this signal.
        let third = session.rebuild(vec![], vec![]);
        assert_eq!(third.shader_dependencies, next.shader_dependencies);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_missing_main_lua_is_explained() {
        let out = temp_dir("missing-main");
        let guard = OutputGuard::for_root(&out);
        let error = prepare_output(&guard, &out).unwrap_err();
        assert!(error.to_string().contains("main.lua must exist"), "{error}");
        fs::remove_dir_all(out).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn dev_files_are_not_written_through_symlinks() {
        use std::os::unix::fs::symlink;

        let root = temp_dir("symlink");
        let out = root.join("build");
        fs::create_dir(&out).unwrap();
        fs::write(out.join("main.lua"), "print('hi')\n").unwrap();
        let victim = root.join("victim.txt");
        fs::write(&victim, "keep me\n").unwrap();
        symlink(&victim, out.join(RUNTIME_FILE)).unwrap();
        let guard = OutputGuard::for_root(&root);

        let error = prepare_output(&guard, &out).unwrap_err();

        assert!(error.to_string().contains("symbolic link"), "{error}");
        assert_eq!(fs::read_to_string(&victim).unwrap(), "keep me\n");
        fs::remove_dir_all(root).unwrap();
    }
}
