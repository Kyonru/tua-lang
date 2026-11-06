//! Capture test output without retaining an unbounded log in the CLI process.
use std::io::{self, Read};
use std::process::{Command, Output, Stdio};

const MAX_STREAM_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn capture(command: &mut Command) -> io::Result<(Output, bool)> {
    capture_with_limit(command, MAX_STREAM_BYTES)
}

fn capture_with_limit(command: &mut Command, limit: usize) -> io::Result<(Output, bool)> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    std::thread::scope(|scope| {
        let out = scope.spawn(|| read_capped(stdout, limit));
        let err = scope.spawn(|| read_capped(stderr, limit));
        let status = child.wait()?;
        let (stdout, out_truncated) = out.join().expect("output reader panicked")?;
        let (stderr, err_truncated) = err.join().expect("output reader panicked")?;
        Ok((
            Output {
                status,
                stdout,
                stderr,
            },
            out_truncated || err_truncated,
        ))
    })
}

fn read_capped(mut stream: impl Read, limit: usize) -> io::Result<(Vec<u8>, bool)> {
    let mut captured = Vec::new();
    let mut truncated = false;
    let mut buffer = [0; 8192];
    loop {
        let count = match stream.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            other => other?,
        };
        if count == 0 {
            break;
        }
        let retained = count.min(limit.saturating_sub(captured.len()));
        captured.extend_from_slice(&buffer[..retained]);
        truncated |= retained < count;
        // Continue draining both pipes so a noisy successful test cannot deadlock.
    }
    Ok((captured, truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_limit_preserves_small_output_and_drains_large_output() {
        assert_eq!(
            read_capped(&b"hello"[..], 8).unwrap(),
            (b"hello".to_vec(), false)
        );
        let input = vec![b'x'; 65536];
        assert_eq!(
            read_capped(input.as_slice(), 64).unwrap(),
            (vec![b'x'; 64], true)
        );
    }

    #[cfg(unix)]
    #[test]
    fn capture_drains_both_process_streams_and_preserves_failure_status() {
        let mut command = Command::new("sh");
        command.args(["-c", "i=0; while [ $i -lt 10000 ]; do printf abcdefghijklmnop; printf qrstuvwxyz012345 >&2; i=$((i+1)); done; exit 7"]);
        let (output, truncated) = capture_with_limit(&mut command, 64).unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout.len(), 64);
        assert_eq!(output.stderr.len(), 64);
        assert!(truncated);
    }
}
