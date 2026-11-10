use std::path::PathBuf;

use super::*;
use crate::{AssetLoaderKind, DiagnosticSeverity, SourcePosition, SourceSpan};

// Output recorded from LOVE 11.5 running the validator on the fixtures in
// src/asset/fixtures/shaders.
const LOVE_OUTPUT: &str = r"READY 11.5.0
RESULT 0 invalid Error validating pixel shader:\n\nERROR: 0:4: '=' :  cannot convert from ' const float' to ' temp 3-component vector of float'\nERROR: 0:4: '' : compilation terminated \nERROR: 2 compilation errors.  No code generated.\n\n\n
RESULT 1 invalid Error validating pixel shader:\n\nERROR: 0:4: '=' :  cannot convert from ' const int' to ' temp mediump float'\nERROR: 0:4: '' : compilation terminated \nERROR: 2 compilation errors.  No code generated.\n\n\n
RESULT 2 valid
RESULT 3 rejected bad argument #2 to '?' (missing 'position' or 'effect' function?)
";

#[test]
fn encodes_length_framed_requests() {
    assert_eq!(
        encode_request(7, ShaderTarget::Gles, "vec4 é;"),
        b"SHADER 7 gles 8\nvec4 \xc3\xa9;".to_vec()
    );
    assert_eq!(
        encode_request(1, ShaderTarget::Desktop, "a;\r\nb;\r\n"),
        b"SHADER 1 desktop 6\na;\nb;\n".to_vec()
    );
    assert_eq!(ShaderTarget::parse("desktop"), Some(ShaderTarget::Desktop));
    assert_eq!(ShaderTarget::parse("metal"), None);
}

#[test]
fn reads_love_validation_results() {
    let output = parse_validator_output(LOVE_OUTPUT);

    assert_eq!(output.love_version.as_deref(), Some("11.5.0"));
    assert_eq!(output.fatal, None);
    assert_eq!(
        output.results[&0],
        ShaderValidationOutcome::Invalid(vec![ShaderError {
            stage: Some(ShaderStage::Pixel),
            line: Some(4),
            message: "cannot convert from 'const float' to 'temp 3-component vector of float'"
                .to_owned(),
        }])
    );
    assert_eq!(
        output.results[&1],
        ShaderValidationOutcome::Invalid(vec![ShaderError {
            stage: Some(ShaderStage::Pixel),
            line: Some(4),
            message: "cannot convert from 'const int' to 'temp mediump float'".to_owned(),
        }])
    );
    assert_eq!(output.results[&2], ShaderValidationOutcome::Valid);
    assert_eq!(
        output.results[&3],
        ShaderValidationOutcome::Rejected("missing 'position' or 'effect' function?".to_owned())
    );
}

#[test]
fn compiler_messages_keep_identifier_tokens() {
    assert_eq!(
        clean_compiler_message("'tint' : undeclared identifier"),
        "'tint': undeclared identifier"
    );
    assert_eq!(
        clean_compiler_message("'texture2D' :  no matching overloaded function found"),
        "'texture2D': no matching overloaded function found"
    );
    assert_eq!(
        clean_compiler_message("'' : syntax error, unexpected IDENTIFIER"),
        "syntax error, unexpected IDENTIFIER"
    );
}

#[test]
fn reports_validator_failures_and_unknown_messages() {
    let output = parse_validator_output(
        "RESULT 0 invalid Error validating vertex shader:\\nsomething unexpected\nFATAL boom\\nstack\n",
    );

    assert_eq!(output.fatal.as_deref(), Some("boom\nstack"));
    assert_eq!(
        output.results[&0],
        ShaderValidationOutcome::Invalid(vec![ShaderError {
            stage: Some(ShaderStage::Vertex),
            line: None,
            message: "Error validating vertex shader: something unexpected".to_owned(),
        }])
    );
}

#[test]
fn places_errors_on_the_shader_and_its_loaders() {
    let reference = AssetReference {
        path: PathBuf::from("src/main.tua"),
        span: SourceSpan {
            start: SourcePosition {
                line: 3,
                column: 14,
            },
            end: SourcePosition {
                line: 3,
                column: 40,
            },
        },
        loader_kind: AssetLoaderKind::Shader,
    };
    let output = parse_validator_output(LOVE_OUTPUT);

    let diagnostics = shader_validation_diagnostics(
        Path::new("assets/glow.glsl"),
        "assets/glow.glsl",
        &[reference],
        ShaderTarget::Gles,
        &output.results[&1],
    );

    let summary = diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.file.to_string_lossy().into_owned(),
                diagnostic.line,
                diagnostic.column,
                diagnostic.severity,
                diagnostic.code,
                diagnostic.message.as_str(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        vec![
            (
                "assets/glow.glsl".to_owned(),
                4,
                1,
                DiagnosticSeverity::Error,
                Some(DiagnosticCode::ASSET_SHADER_COMPILE),
                "shader does not compile for OpenGL ES (mobile and web) (pixel stage): cannot convert from 'const int' to 'temp mediump float'",
            ),
            (
                "src/main.tua".to_owned(),
                3,
                14,
                DiagnosticSeverity::Error,
                Some(DiagnosticCode::ASSET_SHADER_COMPILE),
                "shader `assets/glow.glsl` does not compile for OpenGL ES (mobile and web): line 4: cannot convert from 'const int' to 'temp mediump float'",
            ),
        ]
    );

    let rejected = shader_validation_diagnostics(
        Path::new("assets/glow.glsl"),
        "assets/glow.glsl",
        &[],
        ShaderTarget::Desktop,
        &output.results[&3],
    );
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].line, 1);
    assert_eq!(
        rejected[0].message,
        "shader does not compile for desktop: LÖVE rejects this shader: missing 'position' or 'effect' function?"
    );
    assert!(shader_validation_diagnostics(
        Path::new("assets/glow.glsl"),
        "assets/glow.glsl",
        &[],
        ShaderTarget::Desktop,
        &ShaderValidationOutcome::Valid,
    )
    .is_empty());
}

#[test]
fn collection_skips_disabled_and_unused_assets_and_deduplicates_targets() {
    let asset = crate::ProjectAsset {
        path: "wave.glsl".into(),
        relative_path: "wave.glsl".into(),
        category: crate::AssetCategory::Shader,
        extension: "glsl".into(),
        dimensions: None,
        references: vec![crate::AssetReference {
            path: "main.tua".into(),
            span: SourceSpan {
                start: SourcePosition { line: 1, column: 1 },
                end: SourcePosition { line: 1, column: 8 },
            },
            loader_kind: AssetLoaderKind::Shader,
        }],
    };
    let mut inventory = crate::AssetInventory {
        assets: vec![asset],
        diagnostics: vec![],
    };
    let mut config = crate::ShadersConfig {
        targets: vec![ShaderTarget::Desktop, ShaderTarget::Desktop],
        ..Default::default()
    };
    let requests =
        shader_validation_requests(&inventory, &config, |_| Ok("vec4 effect() {}".into())).unwrap();
    assert_eq!(requests.len(), 1);
    assert!(
        shader_validation_requests(&inventory, &config, |_| Err("invalid utf-8".into()))
            .unwrap()
            .is_empty(),
        "an unreadable shader is skipped instead of failing the batch"
    );
    config.validate = crate::ShaderValidationMode::Off;
    assert!(
        shader_validation_requests(&inventory, &config, |_| panic!("disabled must not read"))
            .unwrap()
            .is_empty()
    );
    config.validate = crate::ShaderValidationMode::Auto;
    inventory.assets[0].references.clear();
    assert!(
        shader_validation_requests(&inventory, &config, |_| panic!("unused must not read"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn validation_expands_includes_and_maps_errors_back_to_the_included_file() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    let root = std::env::temp_dir().join(format!(
        "tua-shader-validation-includes-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("shaders/lib")).unwrap();
    let entry = root.join("shaders/effect.glsl");
    let included = root.join("shaders/lib/common.glsl");
    fs::write(
        &entry,
        "#include \"shaders/lib/common.glsl\"\nvec4 effect() { return tint; }\n",
    )
    .unwrap();
    fs::write(&included, "extern vec4 tint;\n").unwrap();
    let inventory = crate::AssetInventory {
        assets: vec![crate::ProjectAsset {
            path: entry.clone(),
            relative_path: "shaders/effect.glsl".into(),
            category: crate::AssetCategory::Shader,
            extension: "glsl".into(),
            dimensions: None,
            references: vec![crate::AssetReference {
                path: root.join("main.tua"),
                span: SourceSpan {
                    start: SourcePosition { line: 1, column: 1 },
                    end: SourcePosition {
                        line: 1,
                        column: 10,
                    },
                },
                loader_kind: AssetLoaderKind::Shader,
            }],
        }],
        diagnostics: vec![],
    };
    let config = crate::ShadersConfig {
        targets: vec![ShaderTarget::Desktop],
        ..Default::default()
    };
    let requests = shader_validation_requests(&inventory, &config, |path| {
        fs::read_to_string(path).map_err(|error| error.to_string())
    })
    .unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].source.contains("extern vec4 tint;"));
    assert_eq!(requests[0].source_map.as_ref().unwrap()[0].path, included);
    let diagnostics = validation_diagnostics(
        &requests,
        &[ShaderValidationOutcome::Invalid(vec![ShaderError {
            stage: Some(ShaderStage::Pixel),
            line: Some(1),
            message: "bad value".into(),
        }])],
    );
    assert_eq!(diagnostics[0].file, included);
    assert_eq!(diagnostics[0].line, 1);
    let _ = fs::remove_dir_all(root);
}
