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
    entry
        .path
        .as_deref()
        .and_then(|raw| map_windows_path(raw, paths.prefix.as_deref()))
        .filter(|path| path.exists())
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
