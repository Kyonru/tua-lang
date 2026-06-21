use std::io::{self, BufRead, Write};

use serde_json::Value;

pub(super) fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();

        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }

        let trimmed = line.trim_end_matches(['\r', '\n']);

        if trimmed.is_empty() {
            break;
        }

        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = Some(value.trim().parse::<usize>().map_err(|source| {
                    io::Error::new(io::ErrorKind::InvalidData, source.to_string())
                })?);
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
    let body = serde_json::to_vec(message)
        .map_err(|source| io::Error::new(io::ErrorKind::InvalidData, source.to_string()))?;

    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}
