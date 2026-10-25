//! Pure LOVE 11.5 wrapping and glslang diagnostic conversion.
use super::{ShaderError, ShaderStage, ShaderTarget, ShaderValidationOutcome};
use std::{collections::BTreeMap, sync::OnceLock};

pub struct ShaderCompileUnit {
    pub stage: ShaderStage,
    pub source: String,
}

pub fn shader_compile_units(
    source: &str,
    target: ShaderTarget,
) -> Result<Vec<ShaderCompileUnit>, String> {
    static HEADERS: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    let headers = HEADERS.get_or_init(|| {
        serde_json::from_str(include_str!("generated/love-11.5.json"))
            .expect("generated shader headers")
    });
    let language = source
        .trim_start()
        .strip_prefix("#pragma language ")
        .map(|rest| {
            rest.split(|c: char| !c.is_ascii_alphanumeric())
                .next()
                .unwrap_or("")
        })
        .unwrap_or("glsl1");
    if !matches!(language, "glsl1" | "glsl3") {
        return Err(format!("Invalid shader language: {language}"));
    }
    let mut stages = Vec::new();
    if has_entry(source, "vec4", "position") {
        stages.push((ShaderStage::Vertex, "vertex"));
    }
    if has_entry(source, "vec4", "effect") {
        stages.push((ShaderStage::Pixel, "pixel"));
    } else if has_entry(source, "void", "effect") {
        stages.push((
            ShaderStage::Pixel,
            if source.contains("love_Canvases") {
                "multicanvas"
            } else {
                "custom"
            },
        ));
    }
    if stages.is_empty() {
        return Err("missing 'position' or 'effect' function?".into());
    }
    Ok(stages
        .into_iter()
        .map(|(stage, kind)| ShaderCompileUnit {
            stage,
            source: format!(
                "{}{source}",
                headers[&format!("{}/{language}/{kind}", target.as_str())]
            ),
        })
        .collect())
}

// Match LOVE's Lua patterns, including its treatment of signatures in comments.
fn has_entry(source: &str, result: &str, name: &str) -> bool {
    source.match_indices(result).any(|(index, _)| {
        let after = &source[index + result.len()..];
        let trimmed = after.trim_start_matches(|c: char| c.is_ascii_whitespace());
        trimmed.len() < after.len()
            && trimmed.strip_prefix(name).is_some_and(|rest| {
                rest.trim_start_matches(|c: char| c.is_ascii_whitespace())
                    .starts_with('(')
            })
    })
}

/// Source string 1 is user code; errors before its #line marker belong to our header.
pub fn parse_glslang_output(
    output: &str,
    stage: ShaderStage,
    success: bool,
) -> Result<ShaderValidationOutcome, String> {
    if success {
        return Ok(ShaderValidationOutcome::Valid);
    }
    let mut errors = Vec::new();
    for line in output.lines() {
        let Some(rest) = line.strip_prefix("ERROR: ") else {
            continue;
        };
        let mut parts = rest.splitn(3, ':');
        let (Some(file), Some(line), Some(message)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(line) = line.trim().parse::<usize>() else {
            continue;
        };
        if file.trim() != "1" {
            return Err(format!(
                "glslang rejected Tua's LOVE 11.5 header: {}",
                message.trim()
            ));
        }
        let Some(message) = super::compiler_error_text(message) else {
            continue;
        };
        errors.push(ShaderError {
            stage: Some(stage),
            line: Some(line.max(1)),
            message,
        });
    }
    if errors.is_empty() {
        return Err(format!(
            "glslang failed without a shader diagnostic: {}",
            output.trim()
        ));
    }
    Ok(ShaderValidationOutcome::Invalid(errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wraps_stages_targets_and_original_user_lines() {
        let code = "#pragma language glsl3\r\nvec4 position(mat4 m, vec4 p) { return p; }\r\nvoid effect() { love_Canvases[0] = vec4(1.0); }";
        let units = shader_compile_units(code, ShaderTarget::Gles).unwrap();
        assert_eq!(units.len(), 2);
        assert!(units[1].source.contains("#define LOVE_MULTI_CANVAS 1"));
        assert!(units[1].source.ends_with(&format!("#line 1 1\n{code}")));
        assert!(units[0].source.starts_with("#version 300 es"));
        let legacy = shader_compile_units(
            "vec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return c; }",
            ShaderTarget::Gles,
        )
        .unwrap();
        assert!(legacy[0].source.starts_with("#version 100"));
        assert!(shader_compile_units("uniform float t;", ShaderTarget::Desktop).is_err());
    }
    #[test]
    fn separates_header_failures_from_user_diagnostics() {
        let result = parse_glslang_output(
            "ERROR: 1:7: bad identifier\nERROR: 1 compilation errors.",
            ShaderStage::Pixel,
            false,
        )
        .unwrap();
        assert!(
            matches!(result, ShaderValidationOutcome::Invalid(errors) if errors[0].line == Some(7))
        );
        // glslang's follow-up line is not a second user error, and messages
        // are cleaned like LOVE's own validator output.
        let result = parse_glslang_output(
            "ERROR: 1:3: '=' :  cannot convert from ' const int' to ' temp float'\nERROR: 1:3: '' : compilation terminated\nERROR: 2 compilation errors.  No code generated.",
            ShaderStage::Pixel,
            false,
        )
        .unwrap();
        assert_eq!(
            result,
            ShaderValidationOutcome::Invalid(vec![ShaderError {
                stage: Some(ShaderStage::Pixel),
                line: Some(3),
                message: "cannot convert from 'const int' to 'temp float'".into(),
            }])
        );
        assert!(
            parse_glslang_output("ERROR: 0:17: header failed", ShaderStage::Vertex, false).is_err()
        );
    }
}
