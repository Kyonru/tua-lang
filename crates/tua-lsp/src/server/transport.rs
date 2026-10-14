use std::io::{self, BufRead, Read, Write};

use serde_json::Value;

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_CONTENT_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();
    let mut header_bytes = 0;

    loop {
        line.clear();
        let remaining = MAX_HEADER_BYTES.saturating_sub(header_bytes);
        let read = (&mut *reader)
            .take(remaining.saturating_add(1) as u64)
            .read_line(&mut line)?;
        if read == 0 {
            return Ok(None);
        }
        if read > remaining {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("LSP headers exceed {MAX_HEADER_BYTES} bytes"),
            ));
        }
        header_bytes += read;

        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break;
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                if content_length.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "duplicate Content-Length header",
                    ));
                }
                let length = value.trim().parse::<usize>().map_err(|source| {
                    io::Error::new(io::ErrorKind::InvalidData, source.to_string())
                })?;
                if length > MAX_CONTENT_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("LSP message exceeds {MAX_CONTENT_BYTES} bytes"),
                    ));
                }
                content_length = Some(length);
            }
        }
    }

    let Some(content_length) = content_length else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing Content-Length header",
        ));
    };
    let mut body = vec![0; content_length];

    reader.read_exact(&mut body)?;

    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source.to_string()))
}

pub(super) fn write_message(writer: &mut impl Write, message: &Value) -> io::Result<()> {
    let is_diagnostics_publication =
        message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics");
    let _trace = is_diagnostics_publication.then(|| {
        tua_core::profile::diagnostics_span(tua_core::profile::DiagnosticsTraceCategory::Publish)
    });
    let body = serde_json::to_vec(message)
        .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source.to_string()))?;

    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;

    drop(_trace);

    if is_diagnostics_publication {
        let uri = message
            .get("params")
            .and_then(|params| params.get("uri"))
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let diagnostic_count = message
            .get("params")
            .and_then(|params| params.get("diagnostics"))
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default();

        tua_core::profile::report_diagnostics_trace(format!(
            "lsp publishDiagnostics uri={uri} diagnostics={diagnostic_count}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn rejects_oversized_content_before_reading_the_body() {
        let input = format!("Content-Length: {}\r\n\r\n", MAX_CONTENT_BYTES + 1);
        let error = read_message(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("exceeds"));
    }

    #[test]
    fn permits_the_maximum_content_length_before_reading_the_body() {
        let input = format!("Content-Length: {MAX_CONTENT_BYTES}\r\n\r\n");
        let error = read_message(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn rejects_oversized_and_duplicate_headers() {
        let oversized = format!("X-Test: {}\r\n", "x".repeat(MAX_HEADER_BYTES));
        let error = read_message(&mut Cursor::new(oversized)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);

        let duplicate = "Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}";
        let error = read_message(&mut Cursor::new(duplicate)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("duplicate"));
    }
}
