//! Builds the code package for Azure Functions.
//!
//! The package is a zip with `host.json`, `serve/function.json`, and the Linux
//! `husker` binary. The binary must be stored with the Unix mode 0755, because
//! the Functions host cannot change modes in the mounted package. Zip tools on
//! Windows store no Unix mode, so this writes the archive itself.
//!
//! ```text
//! cargo run --example package -- <linux binary> <output zip>
//! ```

use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

/// One file of the archive: its name, Unix mode, and content.
struct Entry {
    name: &'static str,
    mode: u32,
    content: Vec<u8>,
}

fn main() -> io::Result<()> {
    let mut arguments = std::env::args().skip(1);
    let (Some(binary), Some(output)) = (arguments.next(), arguments.next()) else {
        eprintln!("Usage: package <linux binary> <output zip>");
        std::process::exit(2);
    };
    let deploy = Path::new(env!("CARGO_MANIFEST_DIR")).join("deploy/azure");
    let entries = [
        Entry {
            name: "host.json",
            mode: 0o644,
            content: std::fs::read(deploy.join("host.json"))?,
        },
        Entry {
            name: "serve/function.json",
            mode: 0o644,
            content: std::fs::read(deploy.join("serve/function.json"))?,
        },
        Entry {
            name: "husker",
            mode: 0o755,
            content: std::fs::read(&binary)?,
        },
    ];
    let archive = zip(&entries);
    File::create(&output)?.write_all(&archive)?;
    println!("Wrote {output} ({} bytes).", archive.len());
    Ok(())
}

/// Writes a zip archive with stored entries that carry Unix modes.
fn zip(entries: &[Entry]) -> Vec<u8> {
    // Version 2.0, made on Unix: extractors then read the mode from the external attributes.
    const MADE_BY: u16 = (3 << 8) | 20;
    const NEEDED: u16 = 20;
    // 1980-01-01 00:00, the earliest time the format can hold. It keeps the package reproducible.
    const TIME: u16 = 0;
    const DATE: u16 = 0x21;

    let mut archive = Vec::new();
    let mut directory = Vec::new();
    for entry in entries {
        let offset = archive.len() as u32;
        let checksum = crc32(&entry.content);
        let size = entry.content.len() as u32;
        let name = entry.name.as_bytes();

        archive.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
        for field in [NEEDED, 0, 0, TIME, DATE] {
            archive.extend_from_slice(&field.to_le_bytes());
        }
        for field in [checksum, size, size] {
            archive.extend_from_slice(&field.to_le_bytes());
        }
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&0_u16.to_le_bytes());
        archive.extend_from_slice(name);
        archive.extend_from_slice(&entry.content);

        directory.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
        for field in [MADE_BY, NEEDED, 0, 0, TIME, DATE] {
            directory.extend_from_slice(&field.to_le_bytes());
        }
        for field in [checksum, size, size] {
            directory.extend_from_slice(&field.to_le_bytes());
        }
        // Name length, extra length, comment length, disk number, internal attributes.
        for field in [name.len() as u16, 0, 0, 0, 0] {
            directory.extend_from_slice(&field.to_le_bytes());
        }
        // A regular file with the entry's mode, in the high half of the external attributes.
        directory.extend_from_slice(&((0o100_000 | entry.mode) << 16).to_le_bytes());
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(name);
    }

    let directory_offset = archive.len() as u32;
    archive.extend_from_slice(&directory);
    archive.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    for field in [0, 0, entries.len() as u16, entries.len() as u16] {
        archive.extend_from_slice(&field.to_le_bytes());
    }
    archive.extend_from_slice(&(directory.len() as u32).to_le_bytes());
    archive.extend_from_slice(&directory_offset.to_le_bytes());
    archive.extend_from_slice(&0_u16.to_le_bytes());
    archive
}

/// The CRC-32 checksum that the zip format uses.
fn crc32(bytes: &[u8]) -> u32 {
    let mut checksum = !0_u32;
    for byte in bytes {
        checksum ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (checksum & 1).wrapping_neg();
            checksum = (checksum >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !checksum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checksum_matches_the_standard_test_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn the_archive_stores_the_unix_mode_of_each_entry() {
        let archive = zip(&[
            Entry {
                name: "host.json",
                mode: 0o644,
                content: b"{}".to_vec(),
            },
            Entry {
                name: "husker",
                mode: 0o755,
                content: b"binary".to_vec(),
            },
        ]);
        assert_eq!(&archive[..4], b"PK\x03\x04");
        let directory = archive
            .windows(4)
            .rposition(|window| window == b"PK\x01\x02")
            .unwrap();
        let entry = &archive[directory..];
        assert_eq!(entry[5], 3, "made on Unix");
        let attributes = u32::from_le_bytes(entry[38..42].try_into().unwrap());
        assert_eq!(attributes >> 16, 0o100_755);
        assert_eq!(&entry[46..52], b"husker");
        let end = archive.len() - 22;
        assert_eq!(&archive[end..end + 4], b"PK\x05\x06");
        assert_eq!(
            u16::from_le_bytes(archive[end + 10..end + 12].try_into().unwrap()),
            2
        );
    }
}
