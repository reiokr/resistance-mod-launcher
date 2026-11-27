//! Custom profile cover images stored locally in the app data directory.

use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_COVER_BYTES: u64 = 10 * 1024 * 1024;
const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];

#[derive(Debug, Clone, Serialize)]
pub struct ProfileCover {
    pub name: String,
    pub data_url: String,
}

fn covers_dir() -> Result<PathBuf> {
    Ok(crate::app_data_dir()?.join("profile-covers"))
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| ALLOWED_EXTENSIONS.contains(&value.as_str()))
}

fn mime_for(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

fn data_url(path: &Path, ext: &str) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("Could not read {}", path.display()))?;
    Ok(format!(
        "data:{};base64,{}",
        mime_for(ext),
        STANDARD.encode(bytes)
    ))
}

fn cover_path(filename: &str) -> Result<PathBuf> {
    let name = Path::new(filename)
        .file_name()
        .and_then(|value| value.to_str())
        .context("Invalid cover image name.")?;
    Ok(covers_dir()?.join(name))
}

pub fn list() -> Result<Vec<ProfileCover>> {
    let state = crate::state::load()?;
    let mut covers = Vec::new();
    for profile in &state.profiles {
        let Some(filename) = profile.cover_image.as_deref() else {
            continue;
        };
        let path = cover_path(filename)?;
        let Some(ext) = extension(&path) else {
            continue;
        };
        if path.is_file() {
            if let Ok(data_url) = data_url(&path, &ext) {
                covers.push(ProfileCover {
                    name: profile.name.clone(),
                    data_url,
                });
            }
        }
    }
    Ok(covers)
}

pub fn set(name: &str, source: &str) -> Result<ProfileCover> {
    let source = Path::new(source);
    let metadata =
        fs::metadata(source).with_context(|| format!("Could not read {}", source.display()))?;
    if !metadata.is_file() {
        bail!("The selected cover is not a file.");
    }
    if metadata.len() > MAX_COVER_BYTES {
        bail!("The cover image is too large (max 10 MB).");
    }
    let ext = extension(source).context("Unsupported cover format. Use PNG, JPG or WebP.")?;
    let dir = covers_dir()?;
    fs::create_dir_all(&dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    let filename = format!("cover-{stamp}.{ext}");
    let target = dir.join(&filename);
    fs::copy(source, &target)
        .with_context(|| format!("Could not copy cover to {}", target.display()))?;

    let mut state = crate::state::load()?;
    let profile = state
        .profiles
        .iter_mut()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    if let Some(old) = profile.cover_image.as_deref() {
        if let Ok(path) = cover_path(old) {
            let _ = fs::remove_file(path);
        }
    }
    profile.cover_image = Some(filename);
    crate::state::save(&state)?;
    Ok(ProfileCover {
        name: name.to_string(),
        data_url: data_url(&target, &ext)?,
    })
}

pub fn clear(name: &str) -> Result<()> {
    let mut state = crate::state::load()?;
    let profile = state
        .profiles
        .iter_mut()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    if let Some(old) = profile.cover_image.take() {
        if let Ok(path) = cover_path(&old) {
            let _ = fs::remove_file(path);
        }
    }
    crate::state::save(&state)
}
