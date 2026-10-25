//! Conservative declaration metadata for LOVE GLSL, shared by checking and IDE queries.
use crate::{SourcePosition, SourceSpan};
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

pub(crate) const MAX_SHADER_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShaderUniform {
    pub name: String,
    pub glsl_type: String,
    pub array_len: Option<usize>,
    pub array_suffix: String,
    pub declarations: Vec<SourceSpan>,
    pub declaration: String,
    pub read: bool,
    pub initialized: bool,
}
impl ShaderUniform {
    pub(crate) fn detail(&self) -> String {
        format!("{}{}", self.glsl_type, self.array_suffix)
    }
    pub(crate) fn lua_value(&self) -> &'static str {
        match self.glsl_type.as_str() {
            "float" | "number" | "int" | "uint" => "number",
            "bool" => "boolean",
            "vec2" | "ivec2" | "uvec2" => "table with at least 2 numeric components",
            "vec3" | "ivec3" | "uvec3" => "table with at least 3 numeric components",
            "vec4" | "ivec4" | "uvec4" => "table with at least 4 numeric components",
            "bvec2" | "bvec3" | "bvec4" => "table of boolean components",
            "mat2" | "mat3" => "flat numeric table or table of rows",
            "mat4" => "flat numeric table, table of rows, or LOVE Transform",
            "Image" | "sampler2D" => "LOVE Image or compatible Canvas",
            "ArrayImage" | "sampler2DArray" => "LOVE ArrayImage or compatible Canvas",
            "CubeImage" | "samplerCube" => "LOVE CubeImage or compatible Canvas",
            "VolumeImage" | "sampler3D" => "LOVE VolumeImage or compatible Canvas",
            _ => "value supported by LOVE for this GLSL type",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ShaderUniformSet {
    pub uniforms: BTreeMap<String, ShaderUniform>,
    pub names: BTreeSet<String>,
    pub unread: BTreeSet<String>,
    pub has_effect: bool,
    pub has_position: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ShaderSource {
    pub source: Arc<str>,
    pub fingerprint: u64,
    pub uniforms: Option<Arc<ShaderUniformSet>>,
}
impl ShaderSource {
    pub(crate) fn missing_entry_point(&self) -> bool {
        self.uniforms.as_ref().is_some_and(|model| !model.has_effect && !model.has_position)
            // Macro-generated entry points and includes are outside this scanner.
            && !self.source.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with("#define") || line.starts_with("#include")
            })
    }

    pub(crate) fn from_source(source: String, previous: Option<&Self>) -> Option<Self> {
        if source.len() > MAX_SHADER_BYTES {
            return None;
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        source.hash(&mut hash);
        let fingerprint = hash.finish();
        if let Some(previous) =
            previous.filter(|old| old.fingerprint == fingerprint && old.source.as_ref() == source)
        {
            return Some(previous.clone());
        }
        let uniforms = shader_uniforms(&source).map(Arc::new);
        Some(Self {
            source: source.into(),
            fingerprint,
            uniforms,
        })
    }
    pub(crate) fn read(path: &Path, previous: Option<&Self>) -> Option<Self> {
        use std::io::Read;
        let mut source = String::new();
        std::fs::File::open(path)
            .ok()?
            .take(MAX_SHADER_BYTES as u64 + 1)
            .read_to_string(&mut source)
            .ok()?;
        Self::from_source(source, previous)
    }
}

struct Token<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}
fn tokens(source: &str) -> Vec<Token<'_>> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut tokens = Vec::new();
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let start = index;
        if bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_' {
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
        } else {
            index += source[index..].chars().next().unwrap().len_utf8();
        }
        tokens.push(Token {
            text: &source[start..index],
            start,
            end: index,
        });
    }
    tokens
}

/// Comments become same-length blanks, retaining line endings and original byte offsets.
fn strip_comments(source: &str) -> String {
    let mut bytes = source.as_bytes().to_vec();
    let mut index = 0;
    while index + 1 < bytes.len() {
        let end = if &bytes[index..index + 2] == b"//" {
            source[index..]
                .find('\n')
                .map_or(bytes.len(), |length| index + length)
        } else if &bytes[index..index + 2] == b"/*" {
            source[index + 2..]
                .find("*/")
                .map_or(bytes.len(), |length| index + length + 4)
        } else {
            index += 1;
            continue;
        };
        for byte in &mut bytes[index..end] {
            if !matches!(*byte, b'\r' | b'\n') {
                *byte = b' ';
            }
        }
        index = end;
    }
    String::from_utf8(bytes).expect("comments replaced with ASCII blanks")
}
fn position(source: &str, offset: usize) -> SourcePosition {
    let before = &source[..offset];
    SourcePosition {
        line: before.bytes().filter(|byte| *byte == b'\n').count() + 1,
        column: before.rsplit('\n').next().unwrap_or("").chars().count() + 1,
    }
}

pub(crate) fn shader_uniforms(source: &str) -> Option<ShaderUniformSet> {
    let code = strip_comments(source);
    if code.lines().any(|line| {
        line.trim_start().starts_with('#') && (line.contains("uniform") || line.contains("extern"))
    }) {
        return None;
    }
    let tokens = tokens(&code);
    let mut result = ShaderUniformSet::default();
    for window in tokens.windows(3) {
        if window[2].text == "(" {
            result.has_effect |=
                matches!(window[0].text, "vec4" | "void") && window[1].text == "effect";
            result.has_position |= window[0].text == "vec4" && window[1].text == "position";
        }
    }
    let mut index = 0;
    while index < tokens.len() {
        let declaration_start = index;
        let storage = tokens[index].text;
        if !matches!(storage, "extern" | "uniform") {
            index += 1;
            continue;
        }
        index += 1;
        while tokens
            .get(index)
            .is_some_and(|token| matches!(token.text, "lowp" | "mediump" | "highp" | "const"))
        {
            index += 1;
        }
        let glsl_type = tokens.get(index)?.text;
        let header = tokens[declaration_start..=index]
            .iter()
            .map(|token| token.text)
            .collect::<Vec<_>>()
            .join(" ");
        index += 1;
        loop {
            let name = tokens.get(index)?;
            if name.text == "{" {
                return None;
            }
            if !name
                .text
                .starts_with(|character: char| character.is_ascii_alphabetic() || character == '_')
            {
                return None;
            }
            index += 1;
            let mut array_len = None;
            let mut array_suffix = String::new();
            if tokens.get(index).is_some_and(|token| token.text == "[") {
                index += 1;
                let start = index;
                while tokens.get(index).is_some_and(|token| token.text != "]") {
                    index += 1;
                }
                if index == start + 1 {
                    array_len = tokens[start].text.parse().ok();
                }
                tokens.get(index)?;
                array_suffix = format!(
                    "[{}]",
                    tokens[start..index]
                        .iter()
                        .map(|t| t.text)
                        .collect::<String>()
                );
                index += 1;
            }
            let span = SourceSpan {
                start: position(source, name.start),
                end: position(source, name.end),
            };
            let uniform = result
                .uniforms
                .entry(name.text.to_owned())
                .or_insert_with(|| ShaderUniform {
                    name: name.text.to_owned(),
                    glsl_type: glsl_type.to_owned(),
                    array_len,
                    array_suffix: array_suffix.clone(),
                    declarations: Vec::new(),
                    declaration: format!("{header} {}{};", name.text, array_suffix),
                    read: false,
                    initialized: false,
                });
            if uniform.glsl_type != glsl_type || uniform.array_suffix != array_suffix {
                return None;
            }
            uniform.declarations.push(span);
            uniform.initialized |= tokens.get(index).is_some_and(|token| token.text == "=");
            // Initializers are not part of the name list; delimit nested constructors.
            let mut depth = 0usize;
            while let Some(token) = tokens.get(index) {
                match token.text {
                    "{" => return None,
                    "(" | "[" => depth += 1,
                    ")" | "]" => depth = depth.saturating_sub(1),
                    "," | ";" if depth == 0 => break,
                    _ => {}
                }
                index += 1;
            }
            let delimiter = tokens.get(index)?.text;
            index += 1;
            if delimiter == ";" {
                break;
            }
        }
    }
    let mut uses = BTreeMap::<&str, usize>::new();
    for token in &tokens {
        *uses.entry(token.text).or_default() += 1;
    }
    for uniform in result.uniforms.values_mut() {
        uniform.read =
            uses.get(uniform.name.as_str()).copied().unwrap_or(0) > uniform.declarations.len();
        result.names.insert(uniform.name.clone());
        if !uniform.read {
            result.unread.insert(uniform.name.clone());
        }
    }
    Some(result)
}
