//! Shared limits for untrusted textual project inputs.
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_CONFIG_BYTES: usize = 1024 * 1024;
pub const MAX_PROJECT_SOURCE_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_PROJECT_FILES: usize = 20_000;

pub fn read_text(path: &Path, limit: usize) -> io::Result<String> {
    let file = File::open(path)?;
    if file.metadata()?.len() > limit as u64 {
        return Err(io::Error::other(format!(
            "text input exceeds {limit} byte limit"
        )));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::other(format!(
            "text input exceeds {limit} byte limit"
        )));
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn read_source(path: &Path) -> io::Result<String> {
    read_text(path, MAX_SOURCE_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_parsers_enforce_text_limits_before_parsing() {
        let config = " ".repeat(MAX_CONFIG_BYTES + 1);
        assert!(crate::parse_config(&config)
            .unwrap_err()
            .message
            .contains("limit"));
        let source = " ".repeat(MAX_SOURCE_BYTES + 1);
        assert!(crate::parse_tua_source("large.tua", source).unwrap_err()[0]
            .message
            .contains("limit"));
    }

    #[test]
    fn bounded_reader_accepts_boundary_and_rejects_one_byte_over() {
        let path = std::env::temp_dir().join(format!("tua-input-limit-{}", std::process::id()));
        std::fs::write(&path, "abcd").unwrap();
        assert_eq!(read_text(&path, 4).unwrap(), "abcd");
        assert!(read_text(&path, 3)
            .unwrap_err()
            .to_string()
            .contains("limit"));
        std::fs::remove_file(path).unwrap();
    }
}
