//! Native mod metadata catalog.
//!
//! AML's `settings.json` remains the preferred metadata source, but it is no
//! longer required. This catalog caches the merged result in
//! `~/.local/share/xcom2-launcher-linux/mods.json`, so the app keeps working
//! after AML's settings file is deleted or moved. Profiles, categories and
//! feedback stay in `state.json`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use xcom2_launcher_core::{AmlGroup, AmlMod, AmlMods, AmlRoot, SteamPaths};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct NativeCatalog {
    #[serde(default)]
    mods: Vec<AmlMod>,
    #[serde(default)]
    updated: i64,
}

pub fn path() -> Result<PathBuf> {
    Ok(crate::app_data_dir()?.join("mods.json"))
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn load_native() -> Vec<AmlMod> {
    let Ok(path) = path() else {
        return Vec::new();
    };
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str::<NativeCatalog>(&text)
        .map(|catalog| catalog.mods)
        .unwrap_or_default()
}

fn save_native(mods: &[AmlMod]) -> Result<()> {
    let path = path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let catalog = NativeCatalog {
        mods: mods.to_vec(),
        updated: now(),
    };
    let text = serde_json::to_string_pretty(&catalog)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn enrich_missing(existing: &mut AmlMod, native: AmlMod) {
    if existing.name.is_none() {
        existing.name = native.name;
    }
    if existing.author.is_none() {
        existing.author = native.author;
    }
    if existing.description.is_none() {
        existing.description = native.description;
    }
    if existing.workshop_id.unwrap_or(0) == 0 {
        existing.workshop_id = native.workshop_id;
    }
    if existing.path.as_deref().map_or(true, str::is_empty) {
        existing.path = native.path;
    }
    if existing.size == 0 {
        existing.size = native.size;
    }
    if existing.date_updated.is_none() {
        existing.date_updated = native.date_updated;
    }
    if existing.dependencies.is_empty() {
        existing.dependencies = native.dependencies;
    }
    if existing.ignored_dependencies.is_empty() {
        existing.ignored_dependencies = native.ignored_dependencies;
    }
    if !existing.built_for_wotc {
        existing.built_for_wotc = native.built_for_wotc;
    }
}

fn group_mods(mods: Vec<AmlMod>, fallback_category: Option<&str>) -> AmlMods {
    let mut groups: BTreeMap<String, Vec<AmlMod>> = BTreeMap::new();
    for entry in mods {
        let category = entry
            .category
            .clone()
            .or_else(|| fallback_category.map(str::to_string))
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "Unsorted".to_string());
        groups.entry(category).or_default().push(entry);
    }
    let mut entries = BTreeMap::new();
    for (index, (name, mods)) in groups.into_iter().enumerate() {
        entries.insert(
            name,
            AmlGroup {
                index: index as i64,
                entries: mods,
            },
        );
    }
    AmlMods { entries }
}

/// Load AML metadata if available, merge the native catalog, discover new
/// Workshop folders, persist the merged catalog, and return an in-memory
/// `AmlRoot` that the rest of the app can use.
pub fn load_with_paths(paths: &SteamPaths) -> Result<AmlRoot> {
    let aml = paths
        .settings
        .as_ref()
        .filter(|path| path.is_file())
        .map(|path| AmlRoot::load(path))
        .transpose()?;

    let mut all = aml.as_ref().map(AmlRoot::all_mods).unwrap_or_default();
    let mut by_id: HashMap<String, usize> = all
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.id.to_lowercase(), index))
        .collect();

    for native in load_native() {
        let key = native.id.to_lowercase();
        if key.is_empty() {
            continue;
        }
        if let Some(&index) = by_id.get(&key) {
            enrich_missing(&mut all[index], native);
        } else {
            by_id.insert(key, all.len());
            all.push(native);
        }
    }

    let mut settings = aml.unwrap_or_default();
    settings.mods = group_mods(all, None);
    // This also picks up Workshop folders that are not in either source.
    let merged = settings.all_mods_with_workshop(paths.workshop_dir.as_deref())?;
    let fallback_category = Some("Unsorted");
    settings.mods = group_mods(merged.clone(), fallback_category);

    save_native(&merged).context("Could not save native mod catalog")?;
    Ok(settings)
}
