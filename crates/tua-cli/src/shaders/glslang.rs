//! Bounded, headless glslang execution. Only trusted environment/PATH selects the tool.
use super::*;
use tua_core::{
    config::ShaderValidationMode,
    shader_validation::{parse_glslang_output, shader_compile_units, ShaderStage},
};

pub(crate) const NO_GLSLANG: &str = "glslangValidator was not found; install it or set TUA_GLSLANG";
pub(crate) const NO_VALIDATOR: &str = "neither LOVE nor glslangValidator was found";

pub(crate) struct ProjectShaderValidator {
    love: LoveShaderValidator,
    glslang: GlslangValidator,
    love_unavailable: Option<String>,
    observed_tools: Option<String>,
}

impl ProjectShaderValidator {
    pub(crate) fn new(love: Option<&Path>) -> Self {
        Self {
            love: LoveShaderValidator::new(love),
            glslang: GlslangValidator {
                command: std::env::var_os("TUA_GLSLANG")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| "glslangValidator".into()),
                timeout: TIMEOUT,
            },
            love_unavailable: None,
            observed_tools: None,
        }
    }
}

impl ShaderValidator for ProjectShaderValidator {
    fn cache_key(&self) -> String {
        self.love
            .commands
            .iter()
            .chain(std::iter::once(&self.glslang.command))
            .map(|command| tool_identity(command))
            .collect::<Vec<_>>()
            .join("|")
    }
    fn stop(&mut self) {
        self.love.stop();
        self.love_unavailable = None;
    }
    fn validate(
        &mut self,
        requests: &[ShaderValidationRequest],
    ) -> Result<Vec<ShaderValidationOutcome>, String> {
        let key = self.cache_key();
        if self.observed_tools.as_ref() != Some(&key) {
            self.love.stop();
            self.love_unavailable = None;
            self.observed_tools = Some(key);
        }
        let mut outcomes = vec![ShaderValidationOutcome::Valid; requests.len()];
        for mode in [
            ShaderValidationMode::Auto,
            ShaderValidationMode::Love,
            ShaderValidationMode::Glslang,
        ] {
            let indices: Vec<_> = requests
                .iter()
                .enumerate()
                .filter(|(_, r)| r.mode == mode)
                .map(|(i, _)| i)
                .collect();
            if indices.is_empty() {
                continue;
            }
            let batch: Vec<_> = indices.iter().map(|i| requests[*i].clone()).collect();
            let result = if mode == ShaderValidationMode::Glslang {
                self.glslang.validate(&batch)
            } else if mode == ShaderValidationMode::Love {
                self.love.validate(&batch)
            } else {
                let love = match &self.love_unavailable {
                    Some(error) => Err(error.clone()),
                    None => self.love.validate(&batch),
                };
                match love {
                    Ok(outcomes) => Ok(outcomes),
                    Err(error) => {
                        self.love.stop();
                        self.love_unavailable = Some(error.clone());
                        self.glslang.validate(&batch).map_err(|fallback| {
                            if error == crate::love::LOVE_NOT_FOUND && fallback == NO_GLSLANG {
                                NO_VALIDATOR.to_owned()
                            } else {
                                format!("LOVE: {error}; glslang: {fallback}")
                            }
                        })
                    }
                }
            }?;
            for (index, outcome) in indices.into_iter().zip(result) {
                outcomes[index] = outcome;
            }
        }
        Ok(outcomes)
    }
}

fn tool_identity(command: &Path) -> String {
    let paths: Vec<_> = if command.components().count() == 1 {
        std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths)
                    .flat_map(|dir| {
                        let path = dir.join(command);
                        if cfg!(windows) {
                            vec![path.clone(), path.with_extension("exe")]
                        } else {
                            vec![path]
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![command.to_path_buf()]
    };
    for path in paths {
        if let Ok(metadata) = fs::metadata(&path) {
            if metadata.is_file() {
                return format!(
                    "{}:{}:{:?}",
                    path.display(),
                    metadata.len(),
                    metadata.modified()
                );
            }
        }
    }
    format!("missing:{}", command.display())
}

struct GlslangValidator {
    command: PathBuf,
    timeout: Duration,
}
impl GlslangValidator {
    fn validate(
        &mut self,
        requests: &[ShaderValidationRequest],
    ) -> Result<Vec<ShaderValidationOutcome>, String> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let directory = ValidatorDirectory::new().map_err(|e| e.to_string())?;
        let command = if self.command.is_relative() && self.command.components().count() > 1 {
            std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(&self.command)
        } else {
            self.command.clone()
        };
        let mut outcomes = Vec::new();
        let deadline = Instant::now() + self.timeout;
        for request in requests {
            let units = match shader_compile_units(&request.source, request.target) {
                Ok(units) => units,
                Err(message) => {
                    outcomes.push(ShaderValidationOutcome::Rejected(message));
                    continue;
                }
            };
            let mut errors = Vec::new();
            for unit in units {
                let extension = match unit.stage {
                    ShaderStage::Vertex => "vert",
                    ShaderStage::Pixel => "frag",
                };
                let source = directory.0.join(format!("shader.{extension}"));
                fs::write(&source, unit.source).map_err(|e| e.to_string())?;
                let mut process = Command::new(&command);
                process
                    .arg("-S")
                    .arg(extension)
                    .arg(&source)
                    .current_dir(&directory.0)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());
                let mut child = process.spawn().map_err(|e| {
                    if e.kind() == io::ErrorKind::NotFound {
                        NO_GLSLANG.to_owned()
                    } else {
                        format!("cannot start {}: {e}", command.display())
                    }
                })?;
                let stdout = child.stdout.take().expect("piped stdout");
                let stderr = child.stderr.take().expect("piped stderr");
                let (tx, rx) = mpsc::channel();
                for pipe in [Box::new(stdout) as Box<dyn Read + Send>, Box::new(stderr)] {
                    let tx = tx.clone();
                    std::thread::spawn(move || {
                        let mut bytes = Vec::new();
                        let result = pipe
                            .take(MAX_RESPONSE + 1)
                            .read_to_end(&mut bytes)
                            .map(|_| bytes);
                        let _ = tx.send(result);
                    });
                }
                drop(tx);
                let status = loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break status,
                        Ok(None) if Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(10))
                        }
                        result => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(format!(
                                "glslang timed out or failed while waiting: {result:?}"
                            ));
                        }
                    }
                };
                let mut output = Vec::new();
                for _ in 0..2 {
                    let bytes = rx
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .map_err(|_| "glslang output timed out")?
                        .map_err(|e| e.to_string())?;
                    if bytes.len() as u64 > MAX_RESPONSE {
                        return Err("glslang output exceeds 256 KiB".into());
                    }
                    output.extend(bytes);
                    output.push(b'\n');
                }
                if let ShaderValidationOutcome::Invalid(mut stage_errors) = parse_glslang_output(
                    &String::from_utf8_lossy(&output),
                    unit.stage,
                    status.success(),
                )? {
                    errors.append(&mut stage_errors);
                }
            }
            outcomes.push(if errors.is_empty() {
                ShaderValidationOutcome::Valid
            } else {
                ShaderValidationOutcome::Invalid(errors)
            });
        }
        Ok(outcomes)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn fake(body: &str) -> (ValidatorDirectory, PathBuf) {
        let dir = ValidatorDirectory::new().unwrap();
        let command = dir.0.join("validator");
        fs::write(&command, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&command, fs::Permissions::from_mode(0o755)).unwrap();
        (dir, command)
    }
    fn request(mode: ShaderValidationMode) -> ShaderValidationRequest {
        let mut r = super::super::tests::request(
            "vec4 effect(vec4 c, Image t, vec2 u, vec2 p) { return c; }",
        );
        r.mode = mode;
        r
    }
    #[test]
    fn fallback_and_explicit_modes_preserve_original_lines() {
        let (_dir, command) = fake("echo 'ERROR: 1:3: bad shader'; exit 2");
        let mut validator = ProjectShaderValidator::new(Some(Path::new("/no/such/love")));
        validator.glslang.command = command;
        let outcomes = validator
            .validate(&[
                request(ShaderValidationMode::Auto),
                request(ShaderValidationMode::Glslang),
            ])
            .unwrap();
        assert!(outcomes.iter().all(
            |o| matches!(o, ShaderValidationOutcome::Invalid(errors) if errors[0].line == Some(3))
        ));
        assert!(validator
            .validate(&[request(ShaderValidationMode::Love)])
            .is_err());
    }
    #[test]
    fn installing_love_invalidates_fallback_identity_and_retries_preferred_tool() {
        let (dir, glslang) = fake("exit 0");
        let love = dir.0.join("new-love");
        let mut validator = ProjectShaderValidator::new(Some(&love));
        validator.glslang.command = glslang;
        let before = validator.cache_key();
        assert!(matches!(
            validator
                .validate(&[request(ShaderValidationMode::Auto)])
                .unwrap()[0],
            ShaderValidationOutcome::Valid
        ));
        fs::write(&love, "#!/bin/sh\nprintf 'READY 11.5.0\\n'\nwhile read -r command id target size; do\n dd bs=1 count=\"$size\" >/dev/null 2>&1\n printf 'RESULT %s invalid ERROR: 0:2: preferred validator error\\n' \"$id\"\ndone\n").unwrap();
        fs::set_permissions(&love, fs::Permissions::from_mode(0o700)).unwrap();
        assert_ne!(before, validator.cache_key());
        assert!(matches!(
            validator
                .validate(&[request(ShaderValidationMode::Auto)])
                .unwrap()[0],
            ShaderValidationOutcome::Invalid(_)
        ));
    }

    #[test]
    fn headless_validator_timeout_is_bounded() {
        let (_dir, command) = fake("exec sleep 5");
        let mut validator = GlslangValidator {
            command,
            timeout: Duration::from_millis(80),
        };
        let start = Instant::now();
        assert!(validator
            .validate(&[request(ShaderValidationMode::Glslang)])
            .unwrap_err()
            .contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    #[ignore = "requires installed glslangValidator"]
    fn real_glslang_validates_the_shader_corpus() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tua-core/src/asset/fixtures/shaders");
        let mut validator = GlslangValidator {
            command: "glslangValidator".into(),
            timeout: TIMEOUT,
        };
        for entry in fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "glsl") {
                continue;
            }
            let mut r = request(ShaderValidationMode::Glslang);
            r.source = fs::read_to_string(&path).unwrap();
            for target in [
                tua_core::shader_validation::ShaderTarget::Desktop,
                tua_core::shader_validation::ShaderTarget::Gles,
            ] {
                r.target = target;
                let results = validator
                    .validate(&[r.clone()])
                    .unwrap_or_else(|e| panic!("{} {target:?}: {e}", path.display()));
                if path.file_stem().unwrap() == "compile_error" {
                    assert!(
                        matches!(&results[0], ShaderValidationOutcome::Invalid(errors) if errors.iter().any(|e| e.line == Some(4))),
                        "{results:?}"
                    );
                }
                let invalid = matches!(
                    path.file_stem().unwrap().to_str().unwrap(),
                    "compile_error" | "missing_entry_point"
                ) || (path.file_stem().unwrap() == "gles_only_error"
                    && target == tua_core::shader_validation::ShaderTarget::Gles);
                assert_eq!(
                    matches!(results[0], ShaderValidationOutcome::Valid),
                    !invalid,
                    "{} {target:?}: {results:?}",
                    path.display()
                );
            }
        }
    }
}
