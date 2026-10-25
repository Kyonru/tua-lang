//! Isolated, bounded LOVE subprocess used by check and the LSP worker.
mod glslang;
pub(crate) use glslang::{ProjectShaderValidator, NO_GLSLANG, NO_VALIDATOR};

use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::time::{Duration, Instant};
use tua_core::shader_validation::{
    encode_request, parse_validator_output, ShaderValidationOutcome, ShaderValidationRequest,
    ShaderValidator, VALIDATOR_CONF, VALIDATOR_MAIN,
};

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE: u64 = 256 * 1024;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct ValidatorDirectory(PathBuf);
impl ValidatorDirectory {
    fn new() -> io::Result<Self> {
        for _ in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "tua-shaders-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => {
                    let directory = Self(path);
                    fs::write(directory.0.join("main.lua"), VALIDATOR_MAIN)?;
                    fs::write(directory.0.join("conf.lua"), VALIDATOR_CONF)?;
                    return Ok(directory);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::other(
            "could not create shader validator directory",
        ))
    }
}
impl Drop for ValidatorDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Session {
    child: Child,
    requests: SyncSender<Vec<u8>>,
    responses: Receiver<Result<String, String>>,
    _directory: ValidatorDirectory,
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub(crate) struct LoveShaderValidator {
    commands: Vec<PathBuf>,
    session: Option<Session>,
    timeout: Duration,
}
impl LoveShaderValidator {
    pub(crate) fn new(explicit: Option<&Path>) -> Self {
        let mut commands = crate::love::love_commands(explicit);
        // LOVE's Windows console executable provides the protocol pipes.
        if cfg!(windows) {
            for command in &mut commands {
                if command
                    .file_name()
                    .is_some_and(|name| name == "love" || name == "love.exe")
                {
                    command.set_file_name("lovec.exe");
                }
            }
        }
        Self {
            commands,
            session: None,
            timeout: TIMEOUT,
        }
    }

    fn start(&self) -> Result<Session, String> {
        let directory = ValidatorDirectory::new().map_err(|error| error.to_string())?;
        let mut child = None;
        for command in &self.commands {
            // Resolve explicit relative paths before changing cwd. Never load the game.
            let command = if command.is_relative() && command.components().count() > 1 {
                std::env::current_dir()
                    .map_err(|error| error.to_string())?
                    .join(command)
            } else {
                command.clone()
            };
            match Command::new(command)
                .arg(&directory.0)
                .current_dir(&directory.0)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(process) => {
                    child = Some(process);
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(format!("cannot start LOVE shader validator: {error}")),
            }
        }
        let mut child = child.ok_or_else(|| crate::love::LOVE_NOT_FOUND.to_owned())?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (request_tx, request_rx) = mpsc::sync_channel::<Vec<u8>>(1);
        let (response_tx, response_rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok(bytes) = request_rx.recv() {
                if stdin
                    .write_all(&bytes)
                    .and_then(|()| stdin.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                let result = match reader.by_ref().take(MAX_RESPONSE + 1).read_line(&mut line) {
                    Ok(0) => Err("LOVE shader validator exited without a response".to_owned()),
                    Ok(_) if line.len() as u64 > MAX_RESPONSE => {
                        Err("LOVE shader response exceeds limit".to_owned())
                    }
                    Ok(_) => Ok(line),
                    Err(error) => Err(error.to_string()),
                };
                let failed = result.is_err();
                if response_tx.send(result).is_err() || failed {
                    break;
                }
            }
        });
        let session = Session {
            child,
            requests: request_tx,
            responses: response_rx,
            _directory: directory,
        };
        let ready = session
            .responses
            .recv_timeout(self.timeout)
            .map_err(|_| "LOVE shader validator startup timed out".to_owned())??;
        let output = parse_validator_output(&ready);
        if output.love_version.is_none() {
            return Err(output
                .fatal
                .unwrap_or_else(|| "LOVE shader validator did not report READY".to_owned()));
        }
        Ok(session)
    }
}
impl ShaderValidator for LoveShaderValidator {
    fn stop(&mut self) {
        self.session = None;
    }

    fn validate(
        &mut self,
        requests: &[ShaderValidationRequest],
    ) -> Result<Vec<ShaderValidationOutcome>, String> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        if self.session.is_none() {
            self.session = Some(self.start()?);
        }
        let result = (|| {
            let session = self.session.as_ref().expect("started session");
            let deadline = Instant::now() + self.timeout;
            let mut outcomes = Vec::with_capacity(requests.len());
            for (id, request) in requests.iter().enumerate() {
                session
                    .requests
                    .try_send(encode_request(id, request.target, &request.source))
                    .map_err(|_| "LOVE shader validator is not accepting requests".to_owned())?;
                let line = session
                    .responses
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|_| "LOVE shader validation timed out".to_owned())??;
                let mut output = parse_validator_output(&line);
                outcomes.push(output.results.remove(&id).ok_or_else(|| {
                    output
                        .fatal
                        .unwrap_or_else(|| "invalid LOVE shader validator response".to_owned())
                })?);
            }
            Ok(outcomes)
        })();
        if result.is_err() {
            self.session = None;
        }
        result
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tua_core::shader_validation::ShaderTarget;
    use tua_core::{AssetCategory, ProjectAsset};

    pub(super) fn request(source: &str) -> ShaderValidationRequest {
        ShaderValidationRequest {
            mode: tua_core::config::ShaderValidationMode::Auto,
            asset: ProjectAsset {
                path: "test.glsl".into(),
                relative_path: "test.glsl".into(),
                category: AssetCategory::Shader,
                extension: "glsl".into(),
                dimensions: None,
                references: vec![],
            },
            source: source.into(),
            target: ShaderTarget::Desktop,
        }
    }
    fn fake(script: &str) -> (ValidatorDirectory, LoveShaderValidator) {
        let directory = ValidatorDirectory::new().unwrap();
        let path = directory.0.join("fake-love");
        fs::write(&path, format!("#!/bin/sh\n{script}")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        (directory, LoveShaderValidator::new(Some(&path)))
    }
    #[test]
    fn batches_reuse_isolated_process_and_cleanup() {
        let (_fake, mut validator) = fake(
            r#"
[ -f "$1/main.lua" ] && [ "$PWD" -ef "$1" ] || exit 1
printf 'READY 11.5.0\n'
while read -r command id target size; do
  source=$(dd bs=1 count="$size" 2>/dev/null)
  if [ "$source" = broken ]; then
    printf 'RESULT %s invalid ERROR: 0:3: syntax error\n' "$id"
  else
    printf 'RESULT %s valid\n' "$id"
  fi
done
"#,
        );
        let outcomes = validator
            .validate(&[request("valid"), request("broken")])
            .unwrap();
        assert!(matches!(outcomes[0], ShaderValidationOutcome::Valid));
        assert!(matches!(outcomes[1], ShaderValidationOutcome::Invalid(_)));
        let session = validator.session.as_ref().unwrap();
        let pid = session.child.id();
        let directory = session._directory.0.clone();
        validator.validate(&[request("again")]).unwrap();
        assert_eq!(pid, validator.session.as_ref().unwrap().child.id());
        drop(validator);
        assert!(!directory.exists());
    }
    #[test]
    fn hung_validator_is_killed_and_session_discarded() {
        let (_fake, mut validator) = fake("printf 'READY 11.5.0\\n'\nwhile :; do :; done\n");
        validator.timeout = Duration::from_millis(100);
        let start = Instant::now();
        assert!(validator
            .validate(&[request("valid")])
            .unwrap_err()
            .contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(validator.session.is_none());
    }
    #[test]
    fn malformed_response_does_not_become_a_shader_error() {
        let (_fake, mut validator) = fake("printf 'READY 11.5.0\\nRESULT 99 valid\\n'\n");
        assert!(validator
            .validate(&[request("valid")])
            .unwrap_err()
            .contains("invalid LOVE"));
        assert!(validator.session.is_none());
    }
    #[test]
    fn empty_batch_does_not_start_love() {
        let mut validator = LoveShaderValidator::new(Some(Path::new("/no/such/love")));
        assert!(validator.validate(&[]).unwrap().is_empty());
        assert!(validator.session.is_none());
    }
    #[test]
    #[ignore = "requires a graphical session and LOVE; set TUA_LOVE if needed"]
    fn real_love_validates_valid_and_invalid_shaders() {
        let mut validator = LoveShaderValidator::new(None);
        let outcomes = validator
            .validate(&[
                request("vec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return c; }"),
                request("vec4 effect(vec4 c, Image t, vec2 uv, vec2 px) { return missing; }"),
            ])
            .unwrap();
        assert!(matches!(outcomes[0], ShaderValidationOutcome::Valid));
        assert!(matches!(outcomes[1], ShaderValidationOutcome::Invalid(_)));
    }
}
