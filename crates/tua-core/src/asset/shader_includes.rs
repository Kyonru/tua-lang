//! Bounded, source-mapped expansion for project shader `#include` directives.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

const MAX_INCLUDE_FILES: usize = 512;
const MAX_EXPANDED_BYTES: usize = 16 * 1024 * 1024;
const MAX_INCLUDE_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ShaderSourceLocation {
    pub path: PathBuf,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpandedShader {
    pub source: String,
    /// Entry `n - 1` maps expanded source line `n` to its defining file/line.
    pub source_map: Vec<ShaderSourceLocation>,
    pub dependencies: Vec<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShaderIncludeError {
    pub path: PathBuf,
    pub line: usize,
    pub message: String,
}

pub(crate) fn expand_shader_includes(
    root: &Path,
    entry: &Path,
    source: &str,
    mut read: impl FnMut(&Path) -> Result<String, String>,
) -> Result<ExpandedShader, ShaderIncludeError> {
    let root = root.to_path_buf();
    let entry = normalize_project_path(entry).ok_or_else(|| ShaderIncludeError {
        path: entry.to_path_buf(),
        line: 1,
        message: "shader path must stay inside the project root".to_owned(),
    })?;
    let mut state = Expansion::default();
    state.visit(&root, &entry, Some(source), 0, &mut read)?;
    Ok(ExpandedShader {
        source: state.lines.join("\n"),
        source_map: state.source_map,
        dependencies: state.dependencies.into_iter().collect(),
    })
}

#[derive(Default)]
struct Expansion {
    lines: Vec<String>,
    source_map: Vec<ShaderSourceLocation>,
    expanded: BTreeSet<PathBuf>,
    dependencies: BTreeSet<PathBuf>,
    expanded_bytes: usize,
}

impl Expansion {
    fn visit(
        &mut self,
        root: &Path,
        relative: &Path,
        supplied: Option<&str>,
        depth: usize,
        read: &mut impl FnMut(&Path) -> Result<String, String>,
    ) -> Result<(), ShaderIncludeError> {
        if depth > MAX_INCLUDE_DEPTH {
            return Err(ShaderIncludeError {
                path: root.join(relative),
                line: 1,
                message: format!("shader include nesting exceeds {MAX_INCLUDE_DEPTH} files"),
            });
        }
        let relative = normalize_project_path(relative).ok_or_else(|| ShaderIncludeError {
            path: root.join(relative),
            line: 1,
            message: "shader include path must stay inside the project root".to_owned(),
        })?;
        if !self.expanded.insert(relative.clone()) {
            // Include-once semantics keep shared dependencies and cycles finite.
            return Ok(());
        }
        if self.expanded.len() > MAX_INCLUDE_FILES {
            return Err(ShaderIncludeError {
                path: root.join(&relative),
                line: 1,
                message: format!("shader includes exceed the {MAX_INCLUDE_FILES}-file limit"),
            });
        }

        let absolute = root.join(&relative);
        self.dependencies.insert(relative.clone());
        if let (Ok(canonical_root), Ok(canonical_file)) =
            (root.canonicalize(), absolute.canonicalize())
        {
            if !canonical_file.starts_with(&canonical_root) {
                return Err(ShaderIncludeError {
                    path: absolute,
                    line: 1,
                    message: "shader include resolves outside the project root".to_owned(),
                });
            }
        }
        let source = match supplied {
            Some(source) => source.to_owned(),
            None => read(&absolute).map_err(|message| ShaderIncludeError {
                path: absolute.clone(),
                line: 1,
                message: format!("cannot read shader include: {message}"),
            })?,
        };
        self.expanded_bytes = self.expanded_bytes.saturating_add(source.len());
        if self.expanded_bytes > MAX_EXPANDED_BYTES {
            return Err(ShaderIncludeError {
                path: absolute,
                line: 1,
                message: format!("expanded shader exceeds {MAX_EXPANDED_BYTES} bytes"),
            });
        }

        for (index, line) in source.lines().enumerate() {
            let line_number = index + 1;
            match include_directive(line) {
                IncludeLine::NotInclude => {
                    self.lines.push(line.to_owned());
                    self.source_map.push(ShaderSourceLocation {
                        path: absolute.clone(),
                        line: line_number,
                    });
                }
                IncludeLine::Path(path) => {
                    let Some(include) = normalize_project_path(Path::new(path)) else {
                        return Err(ShaderIncludeError {
                            path: absolute.clone(),
                            line: line_number,
                            message: "include path must stay inside the project root".to_owned(),
                        });
                    };
                    let include_absolute = root.join(&include);
                    if let (Ok(canonical_root), Ok(canonical_file)) =
                        (root.canonicalize(), include_absolute.canonicalize())
                    {
                        if !canonical_file.starts_with(&canonical_root) {
                            return Err(ShaderIncludeError {
                                path: absolute.clone(),
                                line: line_number,
                                message: "shader include resolves outside the project root"
                                    .to_owned(),
                            });
                        }
                    }
                    // Include-once skips revisits, including an active cycle edge.
                    if self.expanded.contains(&include) {
                        continue;
                    }
                    if let Err(mut error) = self.visit(root, &include, None, depth + 1, read) {
                        // Attribute a missing child to the directive that requested it.
                        if error.path == include_absolute {
                            error.path = absolute.clone();
                            error.line = line_number;
                            error.message = format!("{}: {}", include.display(), error.message);
                        }
                        return Err(error);
                    }
                }
                IncludeLine::Malformed => {
                    return Err(ShaderIncludeError {
                        path: absolute.clone(),
                        line: line_number,
                        message: "expected `#include \"project/relative/path.glsl\"`".to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}

enum IncludeLine<'a> {
    NotInclude,
    Path(&'a str),
    Malformed,
}

fn include_directive(line: &str) -> IncludeLine<'_> {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix("#include") else {
        return IncludeLine::NotInclude;
    };
    if rest.chars().next().is_some_and(|c| !c.is_whitespace()) {
        return IncludeLine::NotInclude;
    }
    let rest = rest.trim();
    let Some(rest) = rest.strip_prefix('"') else {
        return IncludeLine::Malformed;
    };
    let Some((path, suffix)) = rest.split_once('"') else {
        return IncludeLine::Malformed;
    };
    if path.is_empty() || !suffix.trim().is_empty() {
        return IncludeLine::Malformed;
    }
    IncludeLine::Path(path)
}

fn normalize_project_path(path: &Path) -> Option<PathBuf> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir if !parts.is_empty() => {
                parts.pop();
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "tua-shader-includes-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(path.join("shaders/lib")).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn expansion_is_project_relative_cycle_safe_and_source_mapped() {
        let temp = TempDir::new();
        fs::write(
            temp.0.join("shaders/root.glsl"),
            "#include \"shaders/lib/a.glsl\"\nvec4 effect() { return vec4(value()); }\n",
        )
        .unwrap();
        fs::write(
            temp.0.join("shaders/lib/a.glsl"),
            "#include \"shaders/lib/b.glsl\"\nfloat value() { return nested(); }\n",
        )
        .unwrap();
        fs::write(
            temp.0.join("shaders/lib/b.glsl"),
            "#include \"shaders/lib/a.glsl\"\nfloat nested() { return 1.0; }\n",
        )
        .unwrap();

        let expanded = expand_shader_includes(
            &temp.0,
            Path::new("shaders/root.glsl"),
            &fs::read_to_string(temp.0.join("shaders/root.glsl")).unwrap(),
            |path| fs::read_to_string(path).map_err(|error| error.to_string()),
        )
        .unwrap();

        assert!(expanded.source.contains("float value()"));
        assert!(expanded.source.contains("float nested()"));
        assert_eq!(expanded.source.matches("float value()").count(), 1);
        assert_eq!(expanded.dependencies.len(), 3);
        assert_eq!(
            expanded.source_map[0].path,
            temp.0.join("shaders/lib/b.glsl")
        );
        assert_eq!(expanded.source_map[0].line, 2);
    }

    #[test]
    fn invalid_missing_and_escaping_includes_report_the_directive_location() {
        let temp = TempDir::new();
        let root = temp.0.join("shaders/root.glsl");
        let missing = expand_shader_includes(
            &temp.0,
            Path::new("shaders/root.glsl"),
            "#include \"shaders/lib/missing.glsl\"",
            |path| fs::read_to_string(path).map_err(|error| error.to_string()),
        )
        .unwrap_err();
        assert_eq!(missing.path, root);
        assert_eq!(missing.line, 1);
        assert!(missing.message.contains("shaders/lib/missing.glsl"));

        let escaping = expand_shader_includes(
            &temp.0,
            Path::new("shaders/root.glsl"),
            "#include \"../../outside.glsl\"",
            |_| unreachable!(),
        )
        .unwrap_err();
        assert_eq!(escaping.path, root);
        assert_eq!(escaping.line, 1);
        assert!(escaping.message.contains("project root"));

        let malformed = expand_shader_includes(
            &temp.0,
            Path::new("shaders/root.glsl"),
            "#include <missing.glsl>",
            |_| unreachable!(),
        )
        .unwrap_err();
        assert_eq!(malformed.line, 1);
        assert!(malformed.message.contains("expected `#include"));
    }
}
