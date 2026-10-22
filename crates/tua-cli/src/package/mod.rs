//! `tua package`: build the project and package the output for players.
//!
//! The `.love` archive is always written. Fused Windows and macOS builds
//! combine it with a LOVE runtime the developer supplies with `--love`, so
//! packaging never downloads anything.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use tua_core::{LoadedConfig, ProjectWorkspace, CONFIG_FILE};

use crate::build::run_build;
use crate::error::CliError;

mod zip;

/// Where packages are written, relative to the project root.
const DIST_DIRECTORY: &str = "dist";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PackageTarget {
    #[default]
    Love,
    Windows,
    Macos,
}

impl PackageTarget {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "love" => Some(Self::Love),
            "windows" => Some(Self::Windows),
            "macos" => Some(Self::Macos),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageOptions {
    pub target: PackageTarget,
    pub love: Option<PathBuf>,
    pub name: Option<String>,
    pub identifier: Option<String>,
}

pub(crate) fn run_package(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    options: &PackageOptions,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded.clone());
    let love_runtime = match options.target {
        PackageTarget::Love => None,
        PackageTarget::Windows | PackageTarget::Macos => Some(
            options
                .love
                .clone()
                .ok_or_else(|| missing_runtime(options.target))?,
        ),
    };

    let mut lines = vec![run_build(cwd, loaded, false)?];
    let name = package_name(options.name.as_deref(), workspace.root());
    let dist = workspace.root().join(DIST_DIRECTORY);
    let out_dir = workspace.primary_output_root().to_path_buf();
    let extension = workspace.config().extension.clone();

    let archive = love_archive(&out_dir, &extension, &dist)?;
    fs::create_dir_all(&dist).map_err(|error| io_error(&dist, error))?;
    let love_path = dist.join(format!("{name}.love"));
    fs::write(&love_path, &archive).map_err(|error| io_error(&love_path, error))?;
    lines.push(format!(
        "packaged {} ({})",
        display(cwd, &love_path),
        human_size(archive.len())
    ));

    match (options.target, love_runtime) {
        (PackageTarget::Windows, Some(runtime)) => {
            let directory = fuse_windows(&runtime, &archive, &dist, &name)?;
            lines.push(format!("packaged {}", display(cwd, &directory)));
        }
        (PackageTarget::Macos, Some(runtime)) => {
            let identifier = options
                .identifier
                .clone()
                .unwrap_or_else(|| format!("io.tua.{}", slug(&name)));
            let app = fuse_macos(&runtime, &archive, &dist, &name, &identifier)?;
            lines.push(format!("packaged {}", display(cwd, &app)));
            lines.push(macos_signing_note(&app));
            if options.identifier.is_none() {
                lines.push(format!(
                    "note: the bundle identifier defaults to {identifier}; pass --identifier to use your own"
                ));
            }
        }
        _ => {}
    }

    Ok(lines.join("\n"))
}

fn missing_runtime(target: PackageTarget) -> CliError {
    let hint = match target {
        PackageTarget::Windows => {
            "download the 64-bit zipped LOVE 11.5 for Windows from https://love2d.org, extract it, and pass its directory, which contains love.exe"
        }
        _ => {
            "download LOVE 11.5 for macOS from https://love2d.org and pass its love.app"
        }
    };
    CliError::usage(format!("`--target` needs `--love PATH`: {hint}"))
}

/// The `.love` archive for the build output: every output file except source
/// maps, Tua sources, configuration, packages, and hidden files.
fn love_archive(out_dir: &Path, extension: &str, dist: &Path) -> Result<Vec<u8>, CliError> {
    if !out_dir.join("main.lua").is_file() {
        return Err(CliError::usage(format!(
            "{} has no main.lua; LOVE starts a game from main.lua at the root of the output",
            out_dir.display()
        )));
    }
    let mut entries = Vec::new();
    collect_entries(out_dir, out_dir, extension, dist, &mut entries)
        .map_err(|error| io_error(out_dir, error))?;
    entries.sort();

    let mut archive = zip::ZipWriter::new(Vec::new());
    for (name, path) in entries {
        match path {
            None => archive.add_directory(&name),
            Some(path) => {
                let contents = fs::read(&path).map_err(|error| io_error(&path, error))?;
                archive.add_file(&name, &contents)
            }
        }
        .map_err(|error| io_error(out_dir, error))?;
    }
    archive.finish().map_err(|error| io_error(out_dir, error))
}

type Entry = (String, Option<PathBuf>);

fn collect_entries(
    root: &Path,
    directory: &Path,
    extension: &str,
    dist: &Path,
    entries: &mut Vec<Entry>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let file_type = entry.file_type()?;
        if file_name.starts_with('.') || file_type.is_symlink() || path == dist {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("walk stays under root")
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        if file_type.is_dir() {
            entries.push((format!("{relative}/"), None));
            collect_entries(root, &path, extension, dist, entries)?;
        } else if !is_excluded_file(&file_name, extension) {
            entries.push((relative, Some(path)));
        }
    }
    Ok(())
}

fn is_excluded_file(file_name: &str, extension: &str) -> bool {
    file_name == crate::dev::RUNTIME_FILE
        || file_name.ends_with(".lua.map")
        || file_name.ends_with(".lua.map.json")
        || file_name.ends_with(&format!(".{extension}"))
        || file_name == CONFIG_FILE
}

/// A Windows game: `love.exe` with the archive appended, beside the DLLs and
/// license LOVE ships with.
fn fuse_windows(
    runtime: &Path,
    archive: &[u8],
    dist: &Path,
    name: &str,
) -> Result<PathBuf, CliError> {
    let love_exe = runtime.join("love.exe");
    if !love_exe.is_file() {
        return Err(CliError::usage(format!(
            "{} has no love.exe; pass the extracted LOVE for Windows directory",
            runtime.display()
        )));
    }
    let directory = dist.join("windows").join(name);
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(|error| io_error(&directory, error))?;
    }
    fs::create_dir_all(&directory).map_err(|error| io_error(&directory, error))?;

    let mut executable = fs::read(&love_exe).map_err(|error| io_error(&love_exe, error))?;
    executable.extend_from_slice(archive);
    let target = directory.join(format!("{name}.exe"));
    fs::write(&target, executable).map_err(|error| io_error(&target, error))?;

    for entry in fs::read_dir(runtime).map_err(|error| io_error(runtime, error))? {
        let entry = entry.map_err(|error| io_error(runtime, error))?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let lower = file_name.to_ascii_lowercase();
        if lower.ends_with(".dll") || lower == "license.txt" {
            let target = directory.join(&file_name);
            fs::copy(entry.path(), &target).map_err(|error| io_error(&target, error))?;
        }
    }
    Ok(directory)
}

/// A macOS game: a copy of `love.app` that carries the archive in its
/// resources and names the game in its Info.plist.
fn fuse_macos(
    runtime: &Path,
    archive: &[u8],
    dist: &Path,
    name: &str,
    identifier: &str,
) -> Result<PathBuf, CliError> {
    let plist = runtime.join("Contents/Info.plist");
    if !plist.is_file() {
        return Err(CliError::usage(format!(
            "{} is not a LOVE app bundle; pass the path to love.app",
            runtime.display()
        )));
    }
    let app = dist.join("macos").join(format!("{name}.app"));
    if app.exists() {
        fs::remove_dir_all(&app).map_err(|error| io_error(&app, error))?;
    }
    copy_tree(runtime, &app).map_err(|error| io_error(&app, error))?;

    let resources = app.join("Contents/Resources");
    fs::create_dir_all(&resources).map_err(|error| io_error(&resources, error))?;
    let love_file = resources.join(format!("{name}.love"));
    fs::write(&love_file, archive).map_err(|error| io_error(&love_file, error))?;

    let plist = app.join("Contents/Info.plist");
    let contents = fs::read_to_string(&plist).map_err(|error| io_error(&plist, error))?;
    fs::write(&plist, game_info_plist(&contents, name, identifier))
        .map_err(|error| io_error(&plist, error))?;
    Ok(app)
}

/// Rename the bundle and drop LOVE's claim on `.love` files, as the LOVE
/// distribution guide describes.
fn game_info_plist(plist: &str, name: &str, identifier: &str) -> String {
    let plist = replace_plist_string(plist, "CFBundleName", &xml_escape(name));
    let plist = replace_plist_string(&plist, "CFBundleIdentifier", &xml_escape(identifier));
    remove_plist_key(&plist, "UTExportedTypeDeclarations")
}

fn replace_plist_string(plist: &str, key: &str, value: &str) -> String {
    let marker = format!("<key>{key}</key>");
    let Some(key_at) = plist.find(&marker) else {
        return plist.to_owned();
    };
    let after_key = key_at + marker.len();
    let Some(open) = plist[after_key..].find("<string>").map(|at| after_key + at) else {
        return plist.to_owned();
    };
    let value_start = open + "<string>".len();
    let Some(close) = plist[value_start..]
        .find("</string>")
        .map(|at| value_start + at)
    else {
        return plist.to_owned();
    };
    format!("{}{value}{}", &plist[..value_start], &plist[close..])
}

fn remove_plist_key(plist: &str, key: &str) -> String {
    let marker = format!("<key>{key}</key>");
    let Some(key_at) = plist.find(&marker) else {
        return plist.to_owned();
    };
    let after_key = key_at + marker.len();
    let Some(open) = plist[after_key..].find("<array>").map(|at| after_key + at) else {
        return plist.to_owned();
    };
    // The value is the next array; nested arrays are balanced.
    let mut depth = 0usize;
    let mut cursor = open;
    while cursor < plist.len() {
        if plist[cursor..].starts_with("<array>") {
            depth += 1;
            cursor += "<array>".len();
        } else if plist[cursor..].starts_with("</array>") {
            depth -= 1;
            cursor += "</array>".len();
            if depth == 0 {
                let start = plist[..key_at].trim_end_matches([' ', '\t']).len();
                let end = plist[cursor..]
                    .find('\n')
                    .map_or(cursor, |newline| cursor + newline);
                return format!("{}{}", &plist[..start], &plist[end..]);
            }
        } else {
            cursor += plist[cursor..].chars().next().map_or(1, char::len_utf8);
        }
    }
    plist.to_owned()
}

fn macos_signing_note(app: &Path) -> String {
    if cfg!(target_os = "macos") {
        let signed = Command::new("codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(app)
            .output()
            .is_ok_and(|output| output.status.success());
        if signed {
            return "signed the app ad hoc so it runs on this Mac; sign and notarize it with your Developer ID before distributing it".to_owned();
        }
    }
    "note: changing love.app invalidates its signature; sign it (codesign) and notarize it before distributing".to_owned()
}

fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let to = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            copy_symlink(&from, &to)?;
        } else if file_type.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(fs::read_link(from)?, to)
}

#[cfg(not(unix))]
fn copy_symlink(from: &Path, to: &Path) -> io::Result<()> {
    fs::copy(from, to).map(|_| ())
}

fn package_name(requested: Option<&str>, root: &Path) -> String {
    let raw = requested.map(str::to_owned).unwrap_or_else(|| {
        root.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "game".to_owned())
    });
    let cleaned = raw
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let cleaned = cleaned.trim_matches(['.', ' ']).to_owned();
    if cleaned.is_empty() {
        "game".to_owned()
    } else {
        cleaned
    }
}

fn slug(name: &str) -> String {
    let slug = name
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    if slug.is_empty() {
        "game".to_owned()
    } else {
        slug
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn human_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn display(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

fn io_error(path: &Path, error: io::Error) -> CliError {
    CliError::usage(format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOVE_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>org.love2d.love</string>
	<key>CFBundleName</key>
	<string>LÖVE</string>
	<key>UTExportedTypeDeclarations</key>
	<array>
		<dict>
			<key>UTTypeTagSpecification</key>
			<array>
				<string>love</string>
			</array>
		</dict>
	</array>
	<key>NSHighResolutionCapable</key>
	<true/>
</dict>
</plist>
"#;

    #[test]
    fn plists_name_the_game_and_drop_the_love_file_claim() {
        let plist = game_info_plist(LOVE_PLIST, "Star & Dust", "io.example.stardust");
        assert!(
            plist.contains("<string>Star &amp; Dust</string>"),
            "{plist}"
        );
        assert!(
            plist.contains("<string>io.example.stardust</string>"),
            "{plist}"
        );
        assert!(!plist.contains("UTExportedTypeDeclarations"), "{plist}");
        assert!(!plist.contains("UTTypeTagSpecification"), "{plist}");
        assert!(
            plist.contains("<key>NSHighResolutionCapable</key>"),
            "{plist}"
        );
    }

    #[test]
    fn package_names_are_safe_file_names() {
        assert_eq!(
            package_name(Some("My Game: Deluxe"), Path::new("/x")),
            "My Game- Deluxe"
        );
        assert_eq!(
            package_name(None, Path::new("/projects/star-dust")),
            "star-dust"
        );
        assert_eq!(package_name(Some("..."), Path::new("/x")), "game");
        assert_eq!(slug("My Game: Deluxe"), "mygamedeluxe");
    }

    #[test]
    fn source_maps_sources_and_configuration_stay_out() {
        for excluded in [
            "main.lua.map",
            "main.lua.map.json",
            "player.tua",
            "tua.toml",
        ] {
            assert!(is_excluded_file(excluded, "tua"), "{excluded}");
        }
        for kept in ["main.lua", "conf.lua", "player.png", "map.json"] {
            assert!(!is_excluded_file(kept, "tua"), "{kept}");
        }
    }
}
