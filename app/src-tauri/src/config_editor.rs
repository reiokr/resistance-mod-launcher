//! Mod config editor backend.
//!
//! Lists and edits `Config/**/*.ini` files inside a mod directory. Writes are
//! limited to the mod's own Config tree, preserve the original text encoding
//! and line endings, and create a byte-for-byte backup in the app data dir.

use anyhow::{Context, Result, bail};
use encoding_rs::WINDOWS_1252;
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

use crate::mod_paths::mod_dir;

#[derive(Debug, Clone, Serialize)]
pub struct ConfigFileEntry {
    pub path: String,
    pub size: u64,
    pub modified: Option<i64>,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigFile {
    pub path: String,
    pub content: String,
    pub size: u64,
    pub modified: Option<i64>,
    pub read_only: bool,
    pub encoding: String,
    pub newline: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigSaveResult {
    pub path: String,
    pub backup: Option<String>,
    pub size: u64,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl TextEncoding {
    fn label(self) -> &'static str {
        match self {
            TextEncoding::Utf8 => "UTF-8",
            TextEncoding::Utf8Bom => "UTF-8 BOM",
            TextEncoding::Utf16Le => "UTF-16 LE",
            TextEncoding::Utf16Be => "UTF-16 BE",
            TextEncoding::Windows1252 => "Windows-1252",
        }
    }
}

fn relative_ini_path(relative: &str) -> Result<PathBuf> {
    let normalized = relative.replace('\\', "/");
    if normalized.trim().is_empty() {
        bail!("Config file path is empty.");
    }
    let path = Path::new(&normalized);
    if path.is_absolute() {
        bail!("Absolute paths are not allowed.");
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => bail!("Invalid config path: {relative}"),
        }
    }
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
    {
        bail!("Only .ini files can be edited.");
    }
    Ok(path.to_path_buf())
}

fn resolve_config_path(mod_id: &str, relative: &str) -> Result<(PathBuf, PathBuf)> {
    let relative = relative_ini_path(relative)?;
    let dir = mod_dir(mod_id)?;
    let root = fs::canonicalize(&dir)
        .with_context(|| format!("Could not resolve mod directory: {}", dir.display()))?;
    let config = fs::canonicalize(root.join("Config"))
        .with_context(|| format!("This mod has no Config directory: {mod_id}"))?;
    let full = root.join(&relative);
    let canonical = fs::canonicalize(&full)
        .with_context(|| format!("Config file not found: {}", relative.display()))?;
    if !canonical.starts_with(&config) {
        bail!("Only files under the mod's Config directory can be edited.");
    }
    if !canonical.is_file() {
        bail!("Not a file: {}", relative.display());
    }
    Ok((root, canonical))
}

fn modified_unix(meta: &fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
}

pub fn list_files(mod_id: &str) -> Result<Vec<ConfigFileEntry>> {
    let dir = mod_dir(mod_id)?;
    let root = fs::canonicalize(&dir)
        .with_context(|| format!("Could not resolve mod directory: {}", dir.display()))?;
    let config = root.join("Config");
    if !config.is_dir() {
        return Ok(Vec::new());
    }
    let canonical_config = fs::canonicalize(&config)?;
    let mut files = Vec::new();
    for entry in WalkDir::new(&canonical_config)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ini"))
        {
            continue;
        }
        let Ok(canonical) = fs::canonicalize(path) else {
            continue;
        };
        if !canonical.starts_with(&canonical_config) {
            continue;
        }
        let Ok(relative) = canonical.strip_prefix(&root) else {
            continue;
        };
        let meta = entry.metadata().ok();
        files.push(ConfigFileEntry {
            path: relative.to_string_lossy().replace('\\', "/"),
            size: meta.as_ref().map(|meta| meta.len()).unwrap_or(0),
            modified: meta.as_ref().and_then(modified_unix),
            read_only: meta
                .as_ref()
                .map(|meta| meta.permissions().readonly())
                .unwrap_or(false),
        });
    }
    files.sort_by(|a, b| {
        a.path
            .to_lowercase()
            .cmp(&b.path.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(files)
}

pub fn read(mod_id: &str, relative: &str) -> Result<ConfigFile> {
    let (root, full) = resolve_config_path(mod_id, relative)?;
    let bytes = fs::read(&full).with_context(|| format!("Could not read {}", full.display()))?;
    let (content, encoding) = decode(&bytes);
    let meta = fs::metadata(&full)?;
    let newline = detect_newline(&content);
    let relative = full
        .strip_prefix(&root)
        .unwrap_or(&full)
        .to_string_lossy()
        .replace('\\', "/");
    Ok(ConfigFile {
        path: relative,
        content,
        size: meta.len(),
        modified: modified_unix(&meta),
        read_only: meta.permissions().readonly(),
        encoding: encoding.label().to_string(),
        newline: if newline == "\r\n" { "crlf" } else { "lf" }.to_string(),
    })
}

pub fn save(mod_id: &str, relative: &str, content: &str) -> Result<ConfigSaveResult> {
    let (root, full) = resolve_config_path(mod_id, relative)?;
    let meta = fs::metadata(&full)?;
    if meta.permissions().readonly() {
        bail!("File is read-only: {}", full.display());
    }
    let permissions = meta.permissions();
    let original = fs::read(&full).with_context(|| format!("Could not read {}", full.display()))?;
    let (original_text, encoding) = decode(&original);
    let original_normalized = normalize_newlines(&original_text);
    let new_normalized = normalize_newlines(content);
    let relative_path = full
        .strip_prefix(&root)
        .unwrap_or(&full)
        .to_string_lossy()
        .replace('\\', "/");

    if original_normalized == new_normalized {
        return Ok(ConfigSaveResult {
            path: relative_path,
            backup: None,
            size: original.len() as u64,
            changed: false,
        });
    }

    let newline = detect_newline(&original_text);
    let final_text = apply_newline(&new_normalized, newline);
    if encoding == TextEncoding::Windows1252 {
        let (_, _, had_errors) = WINDOWS_1252.encode(&final_text);
        if had_errors {
            bail!(
                "This file is Windows-1252 and the text contains characters it cannot represent."
            );
        }
    }
    let encoded = encode(&final_text, encoding);
    let backup = backup_path(mod_id, &relative_path)?;
    write_replacing(&full, &encoded, &original, permissions, &backup)?;

    Ok(ConfigSaveResult {
        path: relative_path,
        backup: Some(backup.display().to_string()),
        size: encoded.len() as u64,
        changed: true,
    })
}

fn write_replacing(
    full: &Path,
    bytes: &[u8],
    original: &[u8],
    permissions: fs::Permissions,
    backup: &Path,
) -> Result<()> {
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(backup, original)
        .with_context(|| format!("Could not write backup {}", backup.display()))?;

    let tmp = full.with_extension(format!("ini.tmp-{}", std::process::id()));
    if let Err(err) = fs::write(&tmp, bytes) {
        let _ = fs::remove_file(&tmp);
        return Err(err).with_context(|| format!("Could not write {}", tmp.display()));
    }
    if let Err(err) = fs::set_permissions(&tmp, permissions) {
        let _ = fs::remove_file(&tmp);
        return Err(err)
            .with_context(|| format!("Could not preserve permissions {}", full.display()));
    }
    if let Err(err) = fs::rename(&tmp, full) {
        let _ = fs::remove_file(&tmp);
        return Err(err).with_context(|| format!("Could not replace {}", full.display()));
    }
    Ok(())
}

fn backup_path(mod_id: &str, relative: &str) -> Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let safe_mod: String = mod_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let root = crate::app_data_dir()?.join("backups");
    for attempt in 0..1000 {
        let name = if attempt == 0 {
            format!("config-{stamp}")
        } else {
            format!("config-{stamp}-{attempt}")
        };
        let path = root.join(name).join(&safe_mod).join(relative);
        if !path.exists() {
            return Ok(path);
        }
    }
    bail!("Could not allocate a backup path.")
}

fn decode(bytes: &[u8]) -> (String, TextEncoding) {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        (
            String::from_utf8_lossy(rest).into_owned(),
            TextEncoding::Utf8Bom,
        )
    } else if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        (decode_utf16(rest, true), TextEncoding::Utf16Le)
    } else if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        (decode_utf16(rest, false), TextEncoding::Utf16Be)
    } else {
        match std::str::from_utf8(bytes) {
            Ok(text) => (text.to_string(), TextEncoding::Utf8),
            Err(_) => {
                let (text, _, _) = WINDOWS_1252.decode(bytes);
                (text.into_owned(), TextEncoding::Windows1252)
            }
        }
    }
}

fn encode(text: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut out = Vec::with_capacity(text.len() + 3);
            out.extend_from_slice(b"\xEF\xBB\xBF");
            out.extend_from_slice(text.as_bytes());
            out
        }
        TextEncoding::Utf16Le => encode_utf16(text, true),
        TextEncoding::Utf16Be => encode_utf16(text, false),
        TextEncoding::Windows1252 => {
            let (bytes, _, _) = WINDOWS_1252.encode(text);
            bytes.into_owned()
        }
    }
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

fn encode_utf16(text: &str, little_endian: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + text.len() * 2);
    out.extend_from_slice(if little_endian {
        b"\xFF\xFE"
    } else {
        b"\xFE\xFF"
    });
    for unit in text.encode_utf16() {
        out.extend_from_slice(&if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        });
    }
    out
}

fn detect_newline(text: &str) -> &'static str {
    let crlf = text.matches("\r\n").count();
    let lf = text.matches('\n').count().saturating_sub(crlf);
    if crlf > lf { "\r\n" } else { "\n" }
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn apply_newline(text: &str, newline: &str) -> String {
    if newline == "\r\n" {
        text.replace('\n', "\r\n")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_are_safe() {
        assert!(relative_ini_path("Config/XComGame.ini").is_ok());
        assert!(relative_ini_path("Config\\XComGame.ini").is_ok());
        assert!(relative_ini_path("../XComGame.ini").is_err());
        assert!(relative_ini_path("/etc/passwd.ini").is_err());
        assert!(relative_ini_path("Config/notes.txt").is_err());
    }

    #[test]
    fn utf8_roundtrip() {
        let text = "[Section]\nKey=Value\n";
        let (decoded, encoding) = decode(text.as_bytes());
        assert_eq!(decoded, text);
        assert_eq!(encoding, TextEncoding::Utf8);
        assert_eq!(encode(&decoded, encoding), text.as_bytes());
    }

    #[test]
    fn windows1252_roundtrip() {
        let bytes = [b'[', b'X', b']', b'\r', b'\n', b'K', b'=', 0xE4, 0xF6, 0xFC];
        let (decoded, encoding) = decode(&bytes);
        assert_eq!(encoding, TextEncoding::Windows1252);
        assert!(decoded.contains("äöü"));
        assert_eq!(encode(&decoded, encoding), bytes);
    }

    #[test]
    fn utf16_le_roundtrip() {
        let text = "[X]\r\nK=ä\r\n";
        let bytes = encode_utf16(text, true);
        let (decoded, encoding) = decode(&bytes);
        assert_eq!(decoded, text);
        assert_eq!(encoding, TextEncoding::Utf16Le);
        assert_eq!(encode(&decoded, encoding), bytes);
    }

    #[test]
    fn newline_detection_and_preservation() {
        assert_eq!(detect_newline("a\r\nb\r\n"), "\r\n");
        assert_eq!(detect_newline("a\nb\n"), "\n");
        assert_eq!(apply_newline("a\nb\n", "\r\n"), "a\r\nb\r\n");
        assert_eq!(normalize_newlines("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn write_replaces_file_and_keeps_backup() {
        let base =
            std::env::temp_dir().join(format!("xcom2-config-editor-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let file = base.join("XComGame.ini");
        let backup = base.join("backup/XComGame.ini");
        fs::write(&file, b"old contents\n").unwrap();
        let permissions = fs::metadata(&file).unwrap().permissions();

        write_replacing(
            &file,
            b"new contents\n",
            b"old contents\n",
            permissions,
            &backup,
        )
        .unwrap();

        assert_eq!(fs::read(&file).unwrap(), b"new contents\n");
        assert_eq!(fs::read(&backup).unwrap(), b"old contents\n");
        assert!(
            !file
                .with_extension(format!("ini.tmp-{}", std::process::id()))
                .exists()
        );
        fs::remove_dir_all(&base).unwrap();
    }
}
