//! Portable profile export/import.
//!
//! A profile file contains only the profile definition (mod IDs, order,
//! collection source and explicit deactivations). It never contains machine
//! specific paths or `XComModOptions.ini` contents.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use xcom2_launcher_core::{APP_ID, AmlMod, SteamPaths, discover_paths, scan_workshop_mods};

const FORMAT: &str = "resistance-mod-launcher-profile";
const VERSION: u32 = 1;
const GAME: &str = "xcom2_wotc";
const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;
const MAX_MODS: usize = 5000;
const MAX_NAME_LEN: usize = 200;
const MAX_DESCRIPTION_LEN: usize = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileFile {
    pub format: String,
    pub version: u32,
    #[serde(default)]
    pub exported_at: i64,
    #[serde(default)]
    pub app_version: String,
    #[serde(default = "default_game")]
    pub game: String,
    #[serde(default = "default_app_id")]
    pub workshop_app_id: u32,
    pub profile: ProfileData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileData {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub source: Option<crate::state::ProfileSource>,
    #[serde(default)]
    pub active_mods: Vec<ProfileMod>,
    #[serde(default)]
    pub deactivated_mods: Vec<ProfileMod>,
    #[serde(default)]
    pub pending_workshop_ids: Vec<u64>,
    #[serde(default)]
    pub collection_children: Vec<u64>,
    #[serde(default)]
    pub excluded_workshop_ids: Vec<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMod {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub workshop_id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub file_version: u32,
    pub profile_name: String,
    pub description: Option<String>,
    pub source_title: Option<String>,
    pub source_id: Option<u64>,
    pub active_count: usize,
    pub installed_count: usize,
    pub missing_count: usize,
    pub missing: Vec<ProfileMod>,
    pub deactivated_count: usize,
    pub name_conflict: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub name: String,
    pub active_count: usize,
    pub missing_count: usize,
    pub replaced: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportAllResult {
    pub count: usize,
    pub directory: String,
}

struct ModLookup {
    paths: SteamPaths,
    by_id: HashMap<String, AmlMod>,
    by_workshop: HashMap<u64, AmlMod>,
    installed_internal: HashSet<String>,
}

impl ModLookup {
    fn load() -> Result<Self> {
        let paths = discover_paths()?;
        let settings = crate::catalog::load_with_paths(&paths)?;
        let mut by_id = HashMap::new();
        let mut by_workshop = HashMap::new();
        for entry in settings.all_mods() {
            by_id.insert(entry.id.to_lowercase(), entry.clone());
            if let Some(workshop_id) = entry.workshop_id.filter(|id| *id > 0) {
                by_workshop.entry(workshop_id).or_insert(entry);
            }
        }
        let mut installed_internal = HashSet::new();
        if let Some(workshop) = paths.workshop_dir.as_deref() {
            if let Ok(index) = scan_workshop_mods(workshop) {
                for file in index.by_path.values() {
                    if !file.internal_id.is_empty() {
                        installed_internal.insert(file.internal_id.to_lowercase());
                    }
                }
            }
        }
        Ok(Self {
            paths,
            by_id,
            by_workshop,
            installed_internal,
        })
    }

    fn resolve(&self, item: &ProfileMod) -> Option<&AmlMod> {
        self.by_id
            .get(&item.id.to_lowercase())
            .or_else(|| item.workshop_id.and_then(|id| self.by_workshop.get(&id)))
    }

    fn installed(&self, item: &ProfileMod) -> bool {
        if let Some(id) = item.workshop_id.filter(|id| *id > 0) {
            if let Some(workshop) = &self.paths.workshop_dir {
                if workshop.join(id.to_string()).exists() {
                    return true;
                }
            }
        }
        if self
            .resolve(item)
            .is_some_and(|entry| crate::mod_paths::entry_path(entry, &self.paths).is_some())
        {
            return true;
        }
        self.installed_internal.contains(&item.id.to_lowercase())
            || self
                .resolve(item)
                .is_some_and(|entry| self.installed_internal.contains(&entry.id.to_lowercase()))
    }

    fn exported(&self, mod_id: &str) -> ProfileMod {
        let entry = self.by_id.get(&mod_id.to_lowercase());
        ProfileMod {
            id: mod_id.to_string(),
            workshop_id: entry
                .and_then(|entry| entry.workshop_id)
                .filter(|id| *id > 0),
            name: entry.and_then(|entry| entry.name.clone()),
        }
    }

    fn canonical_id(&self, item: &ProfileMod) -> String {
        self.resolve(item)
            .map(|entry| entry.id.clone())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| item.id.clone())
    }
}

fn default_game() -> String {
    GAME.to_string()
}

fn default_app_id() -> u32 {
    APP_ID
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn dedupe_ids(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect()
}

fn dedupe_u64(values: impl IntoIterator<Item = u64>) -> Vec<u64> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|id| *id > 0 && seen.insert(*id))
        .collect()
}

fn validate(file: &ProfileFile) -> Result<()> {
    if file.format != FORMAT {
        bail!("Not a Resistance Mod Launcher profile file.");
    }
    if file.version != VERSION {
        bail!(
            "Unsupported profile file version: {} (expected {VERSION}).",
            file.version
        );
    }
    if !file.game.eq_ignore_ascii_case(GAME) {
        bail!("This profile is not for XCOM 2: War of the Chosen.");
    }
    if file.workshop_app_id != APP_ID {
        bail!(
            "This profile is for Workshop app {}, not XCOM 2 ({}).",
            file.workshop_app_id,
            APP_ID
        );
    }
    let name = file.profile.name.trim();
    if name.is_empty() {
        bail!("Profile name is empty.");
    }
    if name.len() > MAX_NAME_LEN {
        bail!("Profile name is too long.");
    }
    if file
        .profile
        .description
        .as_deref()
        .is_some_and(|description| description.len() > MAX_DESCRIPTION_LEN)
    {
        bail!("Profile description is too long.");
    }
    if file.profile.active_mods.len() > MAX_MODS || file.profile.deactivated_mods.len() > MAX_MODS {
        bail!("Profile contains too many mods.");
    }
    for item in file
        .profile
        .active_mods
        .iter()
        .chain(file.profile.deactivated_mods.iter())
    {
        if item.id.trim().is_empty() {
            bail!("Profile contains a mod with an empty ID.");
        }
        if item.id.len() > MAX_NAME_LEN {
            bail!("Profile contains a mod ID that is too long.");
        }
        if item
            .name
            .as_deref()
            .is_some_and(|name| name.len() > MAX_NAME_LEN)
        {
            bail!("Profile contains a mod name that is too long.");
        }
    }
    Ok(())
}

fn read_profile_file(path: &Path) -> Result<ProfileFile> {
    let metadata =
        fs::metadata(path).with_context(|| format!("Could not read {}", path.display()))?;
    if metadata.len() > MAX_FILE_SIZE {
        bail!("Profile file is too large.");
    }
    let text =
        fs::read_to_string(path).with_context(|| format!("Could not read {}", path.display()))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let file: ProfileFile = serde_json::from_str(text)
        .with_context(|| format!("Invalid profile file: {}", path.display()))?;
    validate(&file)?;
    Ok(file)
}

fn build_profile(
    file: &ProfileFile,
    name: &str,
    lookup: &ModLookup,
) -> (crate::state::Profile, usize) {
    let active_mods = &file.profile.active_mods;
    let active_ids = dedupe_ids(
        active_mods
            .iter()
            .map(|item| lookup.canonical_id(item))
            .filter(|id| !id.is_empty()),
    );
    let deactivated_mod_ids = dedupe_ids(
        file.profile
            .deactivated_mods
            .iter()
            .map(|item| lookup.canonical_id(item))
            .filter(|id| !id.is_empty()),
    );
    let missing: Vec<u64> = active_mods
        .iter()
        .filter(|item| !lookup.installed(item))
        .filter_map(|item| item.workshop_id.filter(|id| *id > 0))
        .collect();
    let pending_workshop_ids = dedupe_u64(
        file.profile
            .pending_workshop_ids
            .iter()
            .copied()
            .chain(missing.iter().copied()),
    );
    let missing_count = active_mods
        .iter()
        .filter(|item| !lookup.installed(item))
        .count();
    let profile = crate::state::Profile {
        name: name.to_string(),
        description: file.profile.description.clone(),
        active_ids,
        source: file.profile.source.clone(),
        collection_children: file.profile.collection_children.clone(),
        pending_workshop_ids,
        excluded_workshop_ids: file.profile.excluded_workshop_ids.clone(),
        deactivated_mod_ids,
        updated: now(),
    };
    (profile, missing_count)
}

fn build_profile_file(profile: &crate::state::Profile, lookup: &ModLookup) -> ProfileFile {
    ProfileFile {
        format: FORMAT.to_string(),
        version: VERSION,
        exported_at: now(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        game: GAME.to_string(),
        workshop_app_id: APP_ID,
        profile: ProfileData {
            name: profile.name.clone(),
            description: profile.description.clone(),
            source: profile.source.clone(),
            active_mods: profile
                .active_ids
                .iter()
                .map(|id| lookup.exported(id))
                .collect(),
            deactivated_mods: profile
                .deactivated_mod_ids
                .iter()
                .map(|id| lookup.exported(id))
                .collect(),
            pending_workshop_ids: profile.pending_workshop_ids.clone(),
            collection_children: profile.collection_children.clone(),
            excluded_workshop_ids: profile.excluded_workshop_ids.clone(),
        },
    }
}

fn write_profile_file(path: &Path, file: &ProfileFile) -> Result<()> {
    let text = serde_json::to_string_pretty(file)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create {}", parent.display()))?;
        }
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).with_context(|| format!("Could not write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("Could not write {}", path.display()))?;
    Ok(())
}

fn safe_file_name(name: &str) -> String {
    let mut result = String::new();
    for ch in name.chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            result.push('_');
        } else {
            result.push(ch);
        }
    }
    let result = result.trim().trim_matches('.').trim();
    if result.is_empty() {
        "profile".to_string()
    } else {
        result.chars().take(120).collect()
    }
}

pub fn export_profile(name: &str, path: &str) -> Result<()> {
    let state = crate::state::load()?;
    let profile = state
        .profiles
        .iter()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    let lookup = ModLookup::load()?;
    let file = build_profile_file(profile, &lookup);
    write_profile_file(Path::new(path), &file)
}

pub fn export_all_profiles(directory: &str) -> Result<ExportAllResult> {
    let state = crate::state::load()?;
    if state.profiles.is_empty() {
        bail!("There are no profiles to export.");
    }
    let lookup = ModLookup::load()?;
    let directory = Path::new(directory);
    if directory.exists() && !directory.is_dir() {
        bail!("The selected path is not a directory.");
    }
    fs::create_dir_all(directory)
        .with_context(|| format!("Could not create {}", directory.display()))?;
    let mut used_names = HashSet::new();
    let mut count = 0usize;
    for profile in &state.profiles {
        let base = safe_file_name(&profile.name);
        let mut file_name = format!("{base}.x2profile.json");
        let mut suffix = 2usize;
        while !used_names.insert(file_name.to_lowercase()) {
            file_name = format!("{base}-{suffix}.x2profile.json");
            suffix += 1;
        }
        let file = build_profile_file(profile, &lookup);
        write_profile_file(&directory.join(file_name), &file)?;
        count += 1;
    }
    Ok(ExportAllResult {
        count,
        directory: directory.display().to_string(),
    })
}

pub fn preview_profile_import(path: &str) -> Result<ImportPreview> {
    let file = read_profile_file(Path::new(path))?;
    let lookup = ModLookup::load()?;
    let active_count = file.profile.active_mods.len();
    let missing: Vec<ProfileMod> = file
        .profile
        .active_mods
        .iter()
        .filter(|item| !lookup.installed(item))
        .cloned()
        .collect();
    let installed_count = active_count.saturating_sub(missing.len());
    let state = crate::state::load()?;
    let name_conflict = state
        .profiles
        .iter()
        .any(|profile| profile.name == file.profile.name.trim());
    Ok(ImportPreview {
        file_version: file.version,
        profile_name: file.profile.name.trim().to_string(),
        description: file.profile.description.clone(),
        source_title: file
            .profile
            .source
            .as_ref()
            .map(|source| source.title.clone())
            .filter(|title| !title.trim().is_empty()),
        source_id: file.profile.source.as_ref().map(|source| source.id),
        active_count,
        installed_count,
        missing_count: missing.len(),
        missing,
        deactivated_count: file.profile.deactivated_mods.len(),
        name_conflict,
    })
}

pub fn apply_profile_import(
    path: &str,
    name_override: Option<String>,
    replace_existing: bool,
) -> Result<ImportResult> {
    let file = read_profile_file(Path::new(path))?;
    let lookup = ModLookup::load()?;
    let name = name_override
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| file.profile.name.trim())
        .to_string();
    if name.is_empty() {
        bail!("Profile name is empty.");
    }
    if name.len() > MAX_NAME_LEN {
        bail!("Profile name is too long.");
    }
    let mut state = crate::state::load()?;
    let existing = state
        .profiles
        .iter()
        .position(|profile| profile.name == name);
    if existing.is_some() && !replace_existing {
        bail!("Profile already exists: {name}");
    }
    let (profile, missing_count) = build_profile(&file, &name, &lookup);
    let active_count = profile.active_ids.len();
    let replaced = existing.is_some();
    if let Some(index) = existing {
        state.profiles[index] = profile;
    } else {
        state.profiles.push(profile);
    }
    crate::state::save(&state)?;
    Ok(ImportResult {
        name,
        active_count,
        missing_count,
        replaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ProfileFile {
        ProfileFile {
            format: FORMAT.to_string(),
            version: VERSION,
            exported_at: 1,
            app_version: "0.2.4".to_string(),
            game: GAME.to_string(),
            workshop_app_id: APP_ID,
            profile: ProfileData {
                name: "Test profile".to_string(),
                description: Some("Description".to_string()),
                source: None,
                active_mods: vec![ProfileMod {
                    id: "ModA".to_string(),
                    workshop_id: Some(123),
                    name: Some("Mod A".to_string()),
                }],
                deactivated_mods: vec![ProfileMod {
                    id: "ModB".to_string(),
                    workshop_id: None,
                    name: None,
                }],
                pending_workshop_ids: vec![456],
                collection_children: vec![123],
                excluded_workshop_ids: vec![789],
            },
        }
    }

    #[test]
    fn profile_file_round_trips() {
        let text = serde_json::to_string(&sample()).unwrap();
        let parsed: ProfileFile = serde_json::from_str(&text).unwrap();
        validate(&parsed).unwrap();
        assert_eq!(parsed.profile.name, "Test profile");
        assert_eq!(parsed.profile.active_mods[0].id, "ModA");
        assert_eq!(parsed.profile.deactivated_mods[0].id, "ModB");
        assert_eq!(parsed.profile.pending_workshop_ids, vec![456]);
    }

    #[test]
    fn profile_file_rejects_wrong_version_and_format() {
        let mut file = sample();
        file.version = 99;
        assert!(validate(&file).is_err());
        let mut file = sample();
        file.format = "something-else".to_string();
        assert!(validate(&file).is_err());
    }

    #[test]
    fn sanitizes_export_file_names() {
        assert_eq!(safe_file_name("LWOTC - Ted Jam"), "LWOTC - Ted Jam");
        assert_eq!(safe_file_name("a/b:c*?\"<>|"), "a_b_c______");
        assert_eq!(safe_file_name("..."), "profile");
        assert_eq!(safe_file_name(""), "profile");
    }

    #[test]
    fn profile_file_rejects_empty_mod_id() {
        let mut file = sample();
        file.profile.active_mods[0].id.clear();
        assert!(validate(&file).is_err());
    }
}
