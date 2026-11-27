//! Shared mod-directory resolution.

use anyhow::{Context, Result};
use std::path::PathBuf;
use xcom2_launcher_core::{AmlMod, SteamPaths, discover_paths, map_windows_path};

/// Resolve a mod's on-disk directory: Workshop content first, then AML's
/// recorded Windows path mapped through the Proton prefix.
pub fn entry_path(entry: &AmlMod, paths: &SteamPaths) -> Option<PathBuf> {
    if let Some(id) = entry.workshop_id.filter(|id| *id > 0) {
        if let Some(workshop) = &paths.workshop_dir {
            let candidate = workshop.join(id.to_string());
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    // AML sometimes records a folder ID that differs from WorkshopID (for
    // example after a mod is republished or moved between Steam libraries).
    // Prefer that numeric folder inside the currently detected Workshop
    // directory before falling back to a mapped S:\... path.
    if let Some(folder_id) = entry.path.as_deref().and_then(workshop_folder_id) {
        if let Some(workshop) = &paths.workshop_dir {
            let candidate = workshop.join(folder_id.to_string());
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    entry
        .path
        .as_deref()
        .and_then(|raw| map_windows_path(raw, paths.prefix.as_deref()))
        .filter(|path| path.exists())
}

fn workshop_folder_id(raw: &str) -> Option<u64> {
    raw.trim()
        .trim_matches('"')
        .trim_end_matches(['\\', '/'])
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .and_then(|value| value.parse::<u64>().ok())
}

/// Load all configured mods plus newly discovered Workshop folders.
pub fn all_mods() -> Result<Vec<AmlMod>> {
    let paths = discover_paths()?;
    let settings = crate::catalog::load_with_paths(&paths)?;
    Ok(settings.all_mods())
}

/// Find a mod by ID in AML settings and resolve its directory.
pub fn mod_dir(mod_id: &str) -> Result<PathBuf> {
    let paths = discover_paths()?;
    let settings = crate::catalog::load_with_paths(&paths)?;
    let entry = settings
        .all_mods()
        .into_iter()
        .find(|entry| entry.id == mod_id)
        .with_context(|| format!("Mod not found: {mod_id}"))?;
    entry_path(&entry, &paths).with_context(|| format!("Mod directory not found: {mod_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_numeric_workshop_folder_from_windows_path() {
        assert_eq!(
            workshop_folder_id(r"S:\steamapps\workshop\content\268500\3002469584"),
            Some(3002469584)
        );
        assert_eq!(
            workshop_folder_id("/mnt/library/steamapps/workshop/content/268500/123/"),
            Some(123)
        );
        assert_eq!(workshop_folder_id("not-a-number"), None);
    }
}
