//! A minimal, dependency-free ZIP writer for `.love` archives.
//!
//! Entries are stored without compression. LOVE reads stored entries
//! directly, and the bulk of a game (PNG, OGG, fonts) is already compressed.
//! Every entry carries the same timestamp, so identical inputs produce
//! byte-identical archives.

use std::io::{self, Write};

/// 1980-01-01 00:00:00, the earliest DOS timestamp.
const DOS_DATE: u16 = (1 << 5) | 1;
const DOS_TIME: u16 = 0;
const UTF8_NAMES: u16 = 1 << 11;
const VERSION: u16 = 20;
/// Unix permissions in the high half of the external attributes.
const FILE_ATTRIBUTES: u32 = 0o100644 << 16;
const DIRECTORY_ATTRIBUTES: u32 = (0o040755 << 16) | 0x10;

pub(crate) struct ZipWriter<W: Write> {
    out: W,
    offset: u64,
    central: Vec<u8>,
    entries: u64,
}

impl<W: Write> ZipWriter<W> {
    pub(crate) fn new(out: W) -> Self {
        Self {
            out,
            offset: 0,
            central: Vec::new(),
            entries: 0,
        }
    }

    /// Add a directory entry. `name` uses `/` separators and ends with `/`.
    pub(crate) fn add_directory(&mut self, name: &str) -> io::Result<()> {
        debug_assert!(name.ends_with('/'));
        self.add(name, &[], DIRECTORY_ATTRIBUTES)
    }

    /// Add a file entry. `name` uses `/` separators.
    pub(crate) fn add_file(&mut self, name: &str, contents: &[u8]) -> io::Result<()> {
        self.add(name, contents, FILE_ATTRIBUTES)
    }

    fn add(&mut self, name: &str, contents: &[u8], attributes: u32) -> io::Result<()> {
        let name = name.as_bytes();
        let size = u32::try_from(contents.len()).map_err(|_| too_large("entry"))?;
        let name_len = u16::try_from(name.len()).map_err(|_| too_large("entry name"))?;
        let offset = u32::try_from(self.offset).map_err(|_| too_large("archive"))?;
        let crc = crc32(contents);

        let mut local = Vec::with_capacity(30 + name.len());
        local.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        local.extend_from_slice(&VERSION.to_le_bytes());
        local.extend_from_slice(&UTF8_NAMES.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes()); // stored
        local.extend_from_slice(&DOS_TIME.to_le_bytes());
        local.extend_from_slice(&DOS_DATE.to_le_bytes());
        local.extend_from_slice(&crc.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&size.to_le_bytes());
        local.extend_from_slice(&name_len.to_le_bytes());
        local.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        local.extend_from_slice(name);
        self.out.write_all(&local)?;
        self.out.write_all(contents)?;
        self.offset += (local.len() + contents.len()) as u64;

        self.central
            .extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        self.central
            .extend_from_slice(&(VERSION | (3 << 8)).to_le_bytes()); // made by Unix
        self.central.extend_from_slice(&VERSION.to_le_bytes());
        self.central.extend_from_slice(&UTF8_NAMES.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes());
        self.central.extend_from_slice(&DOS_TIME.to_le_bytes());
        self.central.extend_from_slice(&DOS_DATE.to_le_bytes());
        self.central.extend_from_slice(&crc.to_le_bytes());
        self.central.extend_from_slice(&size.to_le_bytes());
        self.central.extend_from_slice(&size.to_le_bytes());
        self.central.extend_from_slice(&name_len.to_le_bytes());
        self.central.extend_from_slice(&0u16.to_le_bytes()); // extra
        self.central.extend_from_slice(&0u16.to_le_bytes()); // comment
        self.central.extend_from_slice(&0u16.to_le_bytes()); // disk
        self.central.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
        self.central.extend_from_slice(&attributes.to_le_bytes());
        self.central.extend_from_slice(&offset.to_le_bytes());
        self.central.extend_from_slice(name);
        self.entries += 1;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> io::Result<W> {
        let entries = u16::try_from(self.entries).map_err(|_| too_large("entry count"))?;
        let central_size = u32::try_from(self.central.len()).map_err(|_| too_large("archive"))?;
        let central_offset = u32::try_from(self.offset).map_err(|_| too_large("archive"))?;
        self.out.write_all(&self.central)?;
        let mut end = Vec::with_capacity(22);
        end.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes());
        end.extend_from_slice(&entries.to_le_bytes());
        end.extend_from_slice(&entries.to_le_bytes());
        end.extend_from_slice(&central_size.to_le_bytes());
        end.extend_from_slice(&central_offset.to_le_bytes());
        end.extend_from_slice(&0u16.to_le_bytes()); // comment length
        self.out.write_all(&end)?;
        self.out.flush()?;
        Ok(self.out)
    }
}

fn too_large(what: &str) -> io::Error {
    io::Error::other(format!(
        "{what} is too large for a non-ZIP64 .love archive (4 GiB / 65535 entries)"
    ))
}

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    const fn table() -> [u32; 256] {
        let mut table = [0u32; 256];
        let mut index = 0;
        while index < 256 {
            let mut value = index as u32;
            let mut bit = 0;
            while bit < 8 {
                value = if value & 1 == 1 {
                    0xEDB8_8320 ^ (value >> 1)
                } else {
                    value >> 1
                };
                bit += 1;
            }
            table[index] = value;
            index += 1;
        }
        table
    }
    const TABLE: [u32; 256] = table();

    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc = TABLE[((crc ^ u32::from(*byte)) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// Names of the entries in an archive written by [`ZipWriter`], with their
/// contents. Used by tests to read archives back without a zip dependency.
#[cfg(test)]
pub(crate) fn read_stored_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let read_u16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
    let read_u32 = |at: usize| {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize
    };
    let mut entries = Vec::new();
    let mut at = 0;
    while read_u32(at) == 0x0403_4b50 {
        let size = read_u32(at + 18);
        let name_len = read_u16(at + 26);
        let extra_len = read_u16(at + 28);
        let name = String::from_utf8(bytes[at + 30..at + 30 + name_len].to_vec()).expect("utf-8");
        let data_start = at + 30 + name_len + extra_len;
        let data = bytes[data_start..data_start + size].to_vec();
        assert_eq!(crc32(&data) as usize, read_u32(at + 14), "crc of {name}");
        entries.push((name, data));
        at = data_start + size;
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn archives_round_trip_and_are_reproducible() {
        let write = || {
            let mut zip = ZipWriter::new(Vec::new());
            zip.add_directory("assets/").unwrap();
            zip.add_file("main.lua", b"print('hi')\n").unwrap();
            zip.add_file("assets/naïve.txt", b"").unwrap();
            zip.finish().unwrap()
        };
        let bytes = write();
        assert_eq!(bytes, write());
        assert_eq!(
            read_stored_entries(&bytes),
            vec![
                ("assets/".to_owned(), Vec::new()),
                ("main.lua".to_owned(), b"print('hi')\n".to_vec()),
                ("assets/naïve.txt".to_owned(), Vec::new()),
            ]
        );
        assert_eq!(&bytes[bytes.len() - 22..bytes.len() - 18], b"PK\x05\x06");
    }
}
