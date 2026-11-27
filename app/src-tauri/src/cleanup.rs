//! Profile deletion impact and Workshop cleanup reports.
//!
//! This module is intentionally read-only. It never deletes mods or edits
//! Steam state; it only explains what a profile uses and which installed
//! Workshop folders are not referenced by any profile or the active list.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use xcom2_launcher_core::{
    AmlMod, SteamPaths, discover_paths, parse_active_mods, scan_workshop_mods,
};

#[derive(Debug, Clone, Serialize)]
pub struct DeletePreview {
    pub name: String,
    pub total: usize,
    pub unique: usize,
    pub shared: usize,
    pub unique_size: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnusedMod {
    pub workshop_id: u64,
    pub published_file_id: Option<u64>,
    pub internal_id: String,
    pub name: String,
    pub size: i64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupReport {
    pub scanned_at: i64,
    pub installed: usize,
    pub referenced: usize,
    pub active: usize,
    pub candidates: Vec<UnusedMod>,
    pub candidate_size: i64,
}

struct Refs {
    internal: HashSet<String>,
    workshop: HashSet<u64>,
}

struct ScanContext {
    paths: SteamPaths,
    by_internal: HashMap<String, AmlMod>,
    workshop_index: HashMap<String, xcom2_launcher_core::ModFile>,
}

impl ScanContext {
    fn load() -> Result<Self> {
        let paths = discover_paths()?;
        let settings = crate::catalog::load_with_paths(&paths)?;
        let mut by_internal = HashMap::new();
        for entry in settings.all_mods() {
            by_internal.insert(entry.id.to_lowercase(), entry);
        }
        let mut workshop_index = HashMap::new();
        if let Some(workshop_dir) = paths.workshop_dir.as_deref() {
            if let Ok(index) = scan_workshop_mods(workshop_dir) {
                for file in index.by_internal_id.values() {
                    workshop_index.insert(file.internal_id.to_lowercase(), file.clone());
                }
            }
        }
        Ok(Self {
            paths,
            by_internal,
            workshop_index,
        })
    }

    fn add_internal_ref(&self, refs: &mut Refs, id: &str) {
        let key = id.to_lowercase();
        refs.internal.insert(key.clone());
        if let Ok(workshop_id) = id.parse::<u64>() {
            refs.workshop.insert(workshop_id);
        }
        if let Some(entry) = self.by_internal.get(&key) {
            if let Some(workshop_id) = entry.workshop_id.filter(|id| *id > 0) {
                refs.workshop.insert(workshop_id);
            }
        }
        if let Some(file) = self.workshop_index.get(&key) {
            if let Some(workshop_id) = file.published_file_id.filter(|id| *id > 0) {
                refs.workshop.insert(workshop_id);
            }
            if let Some(folder) = file_id_folder(&file.path) {
                if let Some(folder_id) = folder_id(&folder) {
                    refs.workshop.insert(folder_id);
                }
            }
        }
    }

    fn active_ids(&self, override_ids: Option<&[String]>) -> Vec<String> {
        let mut ids = Vec::new();
        if let Some(config_dir) = self.paths.wotc_config.as_deref() {
            let path = config_dir.join("XComModOptions.ini");
            if let Ok(text) = fs::read_to_string(&path) {
                ids.extend(parse_active_mods(&text));
            }
        }
        if let Some(overrides) = override_ids {
            ids.extend(overrides.iter().cloned());
        }
        let mut seen = HashSet::new();
        ids.retain(|id| !id.is_empty() && seen.insert(id.clone()));
        ids
    }

    fn refs(&self, exclude_profile: Option<&str>, active_ids: &[String]) -> Refs {
        let mut refs = Refs {
            internal: HashSet::new(),
            workshop: HashSet::new(),
        };
        for id in active_ids {
            self.add_internal_ref(&mut refs, id);
        }
        if let Ok(state) = crate::state::load() {
            for profile in &state.profiles {
                if exclude_profile.is_some_and(|name| name == profile.name) {
                    continue;
                }
                for id in &profile.active_ids {
                    self.add_internal_ref(&mut refs, id);
                }
                for id in &profile.deactivated_mod_ids {
                    self.add_internal_ref(&mut refs, id);
                }
                for id in &profile.pending_workshop_ids {
                    if *id > 0 {
                        refs.workshop.insert(*id);
                    }
                }
            }
        }
        refs
    }

    fn mod_for_id(&self, id: &str) -> Option<&xcom2_launcher_core::ModFile> {
        self.workshop_index.get(&id.to_lowercase())
    }

    fn name_for(&self, file: &xcom2_launcher_core::ModFile) -> String {
        file.title
            .clone()
            .or_else(|| {
                self.by_internal
                    .get(&file.internal_id.to_lowercase())
                    .and_then(|entry| entry.name.clone())
            })
            .unwrap_or_else(|| file.internal_id.clone())
    }
}

fn folder_id(path: &Path) -> Option<u64> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.parse::<u64>().ok())
}

fn directory_size(path: &Path) -> i64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0i64;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            total += directory_size(&entry.path());
        } else if file_type.is_file() {
            total += entry.metadata().map(|meta| meta.len() as i64).unwrap_or(0);
        }
    }
    total
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub fn preview_profile_delete(name: &str) -> Result<DeletePreview> {
    let state = crate::state::load()?;
    let profile = state
        .profiles
        .iter()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    let context = ScanContext::load()?;
    let active_ids = context.active_ids(None);
    let refs = context.refs(Some(name), &active_ids);
    let mut ids = HashSet::new();
    for id in profile
        .active_ids
        .iter()
        .chain(profile.deactivated_mod_ids.iter())
    {
        if !id.is_empty() {
            ids.insert(id.clone());
        }
    }
    let mut unique = 0usize;
    let mut unique_size = 0i64;
    for id in &ids {
        let key = id.to_lowercase();
        let mut used = refs.internal.contains(&key);
        if !used {
            if let Some(file) = context.mod_for_id(id) {
                if let Some(folder_id) = folder_id(&file.path) {
                    used = refs.workshop.contains(&folder_id);
                }
                if !used {
                    if let Some(published) = file.published_file_id {
                        used = refs.workshop.contains(&published);
                    }
                }
            }
        }
        if !used {
            if let Some(entry) = context.by_internal.get(&key) {
                if let Some(workshop_id) = entry.workshop_id.filter(|id| *id > 0) {
                    used = refs.workshop.contains(&workshop_id);
                }
            }
        }
        if !used {
            unique += 1;
            if let Some(file) = context.mod_for_id(id) {
                if let Some(folder) = file_id_folder(&file.path) {
                    if folder.is_dir() {
                        unique_size += directory_size(&folder);
                    }
                }
            } else if let Some(entry) = context.by_internal.get(&key) {
                if let Some(path) = entry
                    .path
                    .as_deref()
                    .and_then(|_| crate::mod_paths::entry_path(entry, &context.paths))
                {
                    unique_size += directory_size(&path);
                }
            }
        }
    }
    Ok(DeletePreview {
        name: profile.name.clone(),
        total: ids.len(),
        unique,
        shared: ids.len().saturating_sub(unique),
        unique_size,
    })
}

pub fn scan_unused_mods(active_ids_override: Option<Vec<String>>) -> Result<CleanupReport> {
    let context = ScanContext::load()?;
    let active_ids = context.active_ids(active_ids_override.as_deref());
    let refs = context.refs(None, &active_ids);
    let active_set: HashSet<String> = active_ids.iter().map(|id| id.to_lowercase()).collect();
    let Some(workshop_dir) = context.paths.workshop_dir.as_deref() else {
        return Ok(CleanupReport {
            scanned_at: now(),
            installed: 0,
            referenced: 0,
            active: 0,
            candidates: Vec::new(),
            candidate_size: 0,
        });
    };
    let index = scan_workshop_mods(workshop_dir)?;
    let mut candidates = Vec::new();
    let mut installed = 0usize;
    let mut active = 0usize;
    let mut seen_paths = HashSet::new();
    for file in index.by_path.values() {
        let Some(folder) = file_id_folder(&file.path) else {
            continue;
        };
        if !seen_paths.insert(folder.clone()) {
            continue;
        }
        installed += 1;
        let key = file.internal_id.to_lowercase();
        let is_active = active_set.contains(&key);
        if is_active {
            active += 1;
        }
        let used_by_profile = refs.internal.contains(&key)
            || file
                .published_file_id
                .is_some_and(|id| refs.workshop.contains(&id))
            || file_id_folder(&file.path)
                .and_then(|folder| folder_id(&folder))
                .is_some_and(|id| refs.workshop.contains(&id));
        if is_active || used_by_profile {
            continue;
        }
        let size = directory_size(&folder);
        candidates.push(UnusedMod {
            workshop_id: folder_id(&folder).unwrap_or(0),
            published_file_id: file.published_file_id,
            internal_id: file.internal_id.clone(),
            name: context.name_for(file),
            size,
            path: folder.display().to_string(),
        });
    }
    candidates.sort_by(|a, b| {
        b.size
            .cmp(&a.size)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let candidate_size = candidates.iter().map(|item| item.size).sum();
    Ok(CleanupReport {
        scanned_at: now(),
        installed,
        referenced: installed.saturating_sub(candidates.len()),
        active,
        candidates,
        candidate_size,
    })
}

fn file_id_folder(path: &Path) -> Option<std::path::PathBuf> {
    if path.is_dir() {
        Some(path.to_path_buf())
    } else {
        path.parent().map(Path::to_path_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_numeric_workshop_folder_from_file_and_directory_paths() {
        let base =
            std::env::temp_dir().join(format!("rml-cleanup-test-{}-{}", std::process::id(), now()));
        let folder = base.join("3348444701");
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("SomeMod.XComMod");
        fs::write(&file, b"test").unwrap();

        assert_eq!(file_id_folder(&file), Some(folder.clone()));
        assert_eq!(folder_id(&folder), Some(3348444701));
        assert_eq!(file_id_folder(&folder), Some(folder));

        fs::remove_dir_all(&base).unwrap();
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QuarantineResult {
    pub workshop_id: u64,
    pub name: String,
    pub size: i64,
}

/// Move an unused Workshop folder to the desktop trash. The folder is
/// re-validated against all profiles and the active list before moving; a
/// referenced mod is never touched.
pub fn quarantine_unused_mod(
    workshop_id: u64,
    active_ids_override: Option<Vec<String>>,
) -> Result<QuarantineResult> {
    if workshop_id == 0 {
        bail!("Invalid Workshop ID.");
    }
    let context = ScanContext::load()?;
    let active_ids = context.active_ids(active_ids_override.as_deref());
    let refs = context.refs(None, &active_ids);
    let active_set: HashSet<String> = active_ids.iter().map(|id| id.to_lowercase()).collect();
    let Some(workshop_dir) = context.paths.workshop_dir.as_deref() else {
        bail!("Workshop directory was not found.");
    };
    let folder = workshop_dir.join(workshop_id.to_string());
    let metadata = fs::symlink_metadata(&folder)
        .with_context(|| format!("Workshop folder not found: {}", folder.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("Refusing to remove a symlinked Workshop folder.");
    }
    if !metadata.is_dir() {
        bail!("Refusing to remove a non-directory Workshop path.");
    }
    if folder.parent() != Some(workshop_dir) {
        bail!("Refusing to remove a path outside the Workshop directory.");
    }
    let index = scan_workshop_mods(workshop_dir)?;
    let file = index
        .by_path
        .values()
        .find(|file| file_id_folder(&file.path).as_deref() == Some(folder.as_path()));
    let internal_id = file
        .map(|file| file.internal_id.to_lowercase())
        .unwrap_or_else(|| workshop_id.to_string());
    let published = file.and_then(|file| file.published_file_id);
    let used = refs.internal.contains(&internal_id)
        || active_set.contains(&internal_id)
        || published.is_some_and(|id| refs.workshop.contains(&id))
        || refs.workshop.contains(&workshop_id);
    if used {
        bail!("This mod is still used by a profile or the active list.");
    }
    let name = file
        .map(|file| context.name_for(file))
        .unwrap_or_else(|| internal_id.clone());
    let size = directory_size(&folder);
    trash::delete(&folder)
        .with_context(|| format!("Could not move {} to trash", folder.display()))?;
    Ok(QuarantineResult {
        workshop_id,
        name,
        size,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct TrashEntry {
    pub id: String,
    pub name: String,
    pub original_path: String,
    pub time_deleted: i64,
}

fn workshop_trash_items() -> Result<(Vec<trash::TrashItem>, std::path::PathBuf)> {
    let paths = discover_paths()?;
    let workshop_dir = paths
        .workshop_dir
        .context("Workshop directory was not found.")?;
    let items = trash::os_limited::list()
        .context("Could not read the system trash.")?
        .into_iter()
        .filter(|item| item.original_parent == workshop_dir)
        .collect();
    Ok((items, workshop_dir))
}

pub fn list_mod_trash() -> Result<Vec<TrashEntry>> {
    let (items, _) = workshop_trash_items()?;
    let mut entries: Vec<TrashEntry> = items
        .into_iter()
        .map(|item| TrashEntry {
            id: item.id.to_string_lossy().to_string(),
            name: item.name.to_string_lossy().to_string(),
            original_path: item.original_path().display().to_string(),
            time_deleted: item.time_deleted,
        })
        .collect();
    entries.sort_by(|a, b| b.time_deleted.cmp(&a.time_deleted));
    Ok(entries)
}

pub fn restore_mod_trash(id: &str) -> Result<TrashEntry> {
    let (items, _) = workshop_trash_items()?;
    let item = items
        .into_iter()
        .find(|item| item.id.to_string_lossy() == id)
        .context("Trash item was not found.")?;
    let entry = TrashEntry {
        id: item.id.to_string_lossy().to_string(),
        name: item.name.to_string_lossy().to_string(),
        original_path: item.original_path().display().to_string(),
        time_deleted: item.time_deleted,
    };
    if item.original_path().exists() {
        bail!(
            "The original folder already exists: {}",
            item.original_path().display()
        );
    }
    trash::os_limited::restore_all(vec![item])
        .with_context(|| format!("Could not restore {}", entry.name))?;
    Ok(entry)
}
