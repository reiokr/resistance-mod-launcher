//! ReadMe viewer backend. Looks for `ReadMe.txt` / `ReadMe.md` in the mod
//! root (case-insensitive) and returns decoded text for the UI.

use anyhow::{Context, Result, bail};
use encoding_rs::WINDOWS_1252;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::mod_paths::mod_dir;

const MAX_README_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct ReadmeContent {
    pub found: bool,
    pub path: Option<String>,
    pub file: Option<String>,
    pub format: String,
    pub content: String,
    pub placeholder: bool,
}

fn find_readme(dir: &Path) -> Option<(PathBuf, &'static str)> {
    let mut text: Option<PathBuf> = None;
    let mut markdown: Option<PathBuf> = None;
    for entry in fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.eq_ignore_ascii_case("readme.txt") {
            text = Some(entry.path());
        } else if name.eq_ignore_ascii_case("readme.md") {
            markdown = Some(entry.path());
        }
    }
    text.map(|path| (path, "text"))
        .or_else(|| markdown.map(|path| (path, "markdown")))
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return decode_utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return decode_utf16(rest, false);
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (text, _, _) = WINDOWS_1252.decode(bytes);
            text.into_owned()
        }
    }
}

pub fn read(mod_id: &str) -> Result<ReadmeContent> {
    let dir = mod_dir(mod_id)?;
    let Some((path, format)) = find_readme(&dir) else {
        return Ok(ReadmeContent {
            found: false,
            path: None,
            file: None,
            format: "text".to_string(),
            content: String::new(),
            placeholder: false,
        });
    };

    let meta = fs::metadata(&path)?;
    if meta.len() > MAX_README_BYTES {
        bail!(
            "ReadMe is too large to display ({} bytes; limit {}).",
            meta.len(),
            MAX_README_BYTES
        );
    }
    let bytes = fs::read(&path).with_context(|| format!("Could not read {}", path.display()))?;
    let mut content = decode(&bytes);
    let placeholder = if format == "text" {
        content.contains("You created an XCOM 2 Mod Project!")
    } else {
        content.contains("Describe specific features of your mod.")
    };
    if placeholder {
        content = "Modi autor ei lisanud ReadMe-sse sisu.".to_string();
    }

    Ok(ReadmeContent {
        found: true,
        path: Some(path.display().to_string()),
        file: path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string()),
        format: format.to_string(),
        content,
        placeholder,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_and_windows1252() {
        assert_eq!(decode("Tere\n".as_bytes()), "Tere\n");
        let bytes = [b'R', b'e', b'a', b'd', b'M', b'e', b':', b' ', 0xE4];
        assert_eq!(decode(&bytes), "ReadMe: ä");
    }

    #[test]
    fn decodes_utf16_le() {
        let text = "ReadMe";
        let mut bytes = b"\xFF\xFE".to_vec();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode(&bytes), text);
    }
}
