use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use tua_core::{LoadedConfig, ProjectWorkspace};

use crate::args::CommandOptions;
use crate::error::CliError;
use crate::source_map::{load_traceback_maps, TracebackSourceMap};

pub(crate) fn run_traceback(
    cwd: impl AsRef<Path>,
    loaded: LoadedConfig,
    options: CommandOptions,
) -> Result<String, CliError> {
    let cwd = cwd.as_ref();
    let workspace = ProjectWorkspace::from_loaded(cwd, loaded);
    let maps_path = options
        .traceback_maps
        .as_deref()
        .map(|path| resolve_cli_path(cwd, path))
        .unwrap_or_else(|| workspace.primary_output_root().to_path_buf());
    let maps = load_traceback_maps(&maps_path)?;
    let input = read_traceback_input(cwd, options.traceback_input.as_deref())?;

    Ok(remap_traceback_text(&input, &maps))
}

fn read_traceback_input(cwd: &Path, input_path: Option<&Path>) -> Result<String, CliError> {
    if let Some(path) = input_path {
        let path = resolve_cli_path(cwd, path);

        return fs::read_to_string(&path).map_err(|source| {
            CliError::usage(format!(
                "failed to read traceback `{}`: {source}",
                display_path(&path)
            ))
        });
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|source| {
        CliError::usage(format!("failed to read traceback from stdin: {source}"))
    })?;

    Ok(input)
}

fn remap_traceback_text(input: &str, maps: &[TracebackSourceMap]) -> String {
    let mut output = input
        .lines()
        .map(|line| remap_traceback_line(line, maps))
        .collect::<Vec<_>>()
        .join("\n");

    if input.ends_with('\n') {
        output.push('\n');
    }

    output
}

fn remap_traceback_line(line: &str, maps: &[TracebackSourceMap]) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;

    while cursor < line.len() {
        let Some(relative_lua) = line[cursor..].find(".lua:") else {
            output.push_str(&line[cursor..]);
            break;
        };
        let lua_start = cursor + relative_lua;
        let path_end = lua_start + ".lua".len();
        let line_start = path_end + 1;
        let line_end = consume_digits(line, line_start);

        if line_end == line_start {
            output.push_str(&line[cursor..line_start]);
            cursor = line_start;
            continue;
        }

        let path_start = traceback_path_start(line, lua_start);
        let generated_path = &line[path_start..path_end];
        let Some(generated_line) = line[line_start..line_end].parse::<usize>().ok() else {
            output.push_str(&line[cursor..line_end]);
            cursor = line_end;
            continue;
        };

        if let Some((source_path, source_line)) =
            lookup_source_location(maps, generated_path, generated_line)
        {
            output.push_str(&line[cursor..path_start]);
            output.push_str(&format!(
                "{}:{} (generated {}:{})",
                source_path, source_line, generated_path, generated_line
            ));
            cursor = line_end;
        } else {
            output.push_str(&line[cursor..line_end]);
            cursor = line_end;
        }
    }

    output
}

fn lookup_source_location<'a>(
    maps: &'a [TracebackSourceMap],
    generated_path: &str,
    generated_line: usize,
) -> Option<(&'a str, usize)> {
    let mut matches = maps
        .iter()
        .filter_map(|map| {
            let source_line = map.lines.get(&generated_line).copied()?;
            let score = path_match_score(generated_path, &map.generated)?;

            Some((score, map.source.as_str(), source_line))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|item| std::cmp::Reverse(item.0));

    if matches.is_empty() {
        return None;
    }

    if matches.len() == 1 || matches[0].0 > matches[1].0 {
        let (_, source_path, source_line) = matches[0];
        return Some((source_path, source_line));
    }

    None
}

fn path_match_score(traceback_path: &str, generated_path: &str) -> Option<usize> {
    let traceback_path = normalize_traceback_path(traceback_path);
    let generated_path = normalize_traceback_path(generated_path);

    if traceback_path == generated_path {
        return Some(generated_path.len() + 10_000);
    }

    if traceback_path.ends_with(&format!("/{generated_path}")) {
        return Some(generated_path.len());
    }

    if generated_path.ends_with(&format!("/{traceback_path}")) {
        return Some(traceback_path.len());
    }

    None
}

fn normalize_traceback_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_owned()
}

fn traceback_path_start(line: &str, lua_start: usize) -> usize {
    let mut path_start = lua_start;

    for (index, character) in line[..lua_start].char_indices().rev() {
        if is_traceback_path_boundary(character) {
            break;
        }

        path_start = index;
    }

    path_start
}

fn is_traceback_path_boundary(character: char) -> bool {
    character.is_whitespace() || matches!(character, '"' | '\'' | '[' | '(')
}

fn consume_digits(input: &str, mut index: usize) -> usize {
    while index < input.len() {
        let character = input[index..]
            .chars()
            .next()
            .expect("index should stay on a valid string boundary");

        if !character.is_ascii_digit() {
            break;
        }

        index += character.len_utf8();
    }

    index
}

fn resolve_cli_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests;
