//! Native Linux core for XCOM 2 mod management.
//!
//! This crate is intentionally UI-free. The first milestone is a read-only
//! prototype that can:
//!
//! * discover the Steam library, the XCOM 2 install and the Proton prefix
//! * import AML's `settings.json`
//! * scan the Steam Workshop content directory
//! * resolve the active mod list and generate `XComModOptions.ini`
//! * compare the generated list with the current game config
//!
//! It does not write to the game config yet.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const APP_ID: u32 = 268500;
pub const WOTC_GAME_DIR: &str = "XCOM2 War of the Chosen";
pub const VANILLA_GAME_DIR: &str = "XCOM2";
pub const WOTC_DEFAULT_DIR: &str = "XCom2-WarOfTheChosen";
pub const VANILLA_DEFAULT_DIR: &str = "XComGame";

// ---------------------------------------------------------------------------
// path discovery
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct SteamPaths {
    pub steam_root: Option<PathBuf>,
    pub library: Option<PathBuf>,
    pub game_dir: Option<PathBuf>,
    pub prefix: Option<PathBuf>,
    pub workshop_dir: Option<PathBuf>,
    pub aml_dir: Option<PathBuf>,
    pub settings: Option<PathBuf>,
    pub wotc_config: Option<PathBuf>,
    pub vanilla_config: Option<PathBuf>,
}

pub fn discover_paths() -> Result<SteamPaths> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")?;

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::var_os("STEAM_DIR") {
        roots.push(PathBuf::from(dir));
    }
    roots.push(home.join(".local/share/Steam"));
    roots.push(home.join(".steam/steam"));
    roots.push(home.join(".steam/root"));
    roots.push(home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"));
    roots.push(home.join("snap/steam/common/.local/share/Steam"));

    let steam_root = roots
        .iter()
        .find(|root| root.join("steamapps").is_dir())
        .cloned();

    let mut libraries: Vec<PathBuf> = Vec::new();
    if let Some(root) = &steam_root {
        libraries.push(root.clone());
        libraries.extend(parse_library_folders(root));
    }

    dedup_paths(&mut libraries);

    let mut candidates: Vec<(u8, PathBuf, PathBuf, PathBuf, PathBuf)> = Vec::new();
    for library in &libraries {
        let game_dir = game_install_dir(library);
        let prefix = library
            .join("steamapps/compatdata")
            .join(APP_ID.to_string())
            .join("pfx");
        let workshop = library
            .join("steamapps/workshop/content")
            .join(APP_ID.to_string());
        let mut score = 0u8;
        if game_dir.is_dir() {
            score += 2;
        }
        if prefix.is_dir() {
            score += 2;
        }
        if workshop.is_dir() {
            score += 1;
        }
        if game_dir.join("Launcher/settings.json").is_file() {
            score += 1;
        }
        if score > 0 {
            candidates.push((score, library.clone(), game_dir, prefix, workshop));
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));

    let mut paths = SteamPaths {
        steam_root,
        ..Default::default()
    };

    if let Some((_, library, game_dir, prefix, workshop)) = candidates.into_iter().next() {
        paths.library = Some(library);
        paths.game_dir = if game_dir.is_dir() {
            Some(game_dir)
        } else {
            None
        };
        paths.prefix = if prefix.is_dir() { Some(prefix) } else { None };
        paths.workshop_dir = if workshop.is_dir() {
            Some(workshop)
        } else {
            None
        };
    }

    if let Some(game_dir) = &paths.game_dir {
        let aml_dir = game_dir.join("Launcher");
        if aml_dir.is_dir() {
            paths.aml_dir = Some(aml_dir);
            let settings = paths.aml_dir.as_ref().unwrap().join("settings.json");
            if settings.is_file() {
                paths.settings = Some(settings);
            }
        }
    }

    if let Some(prefix) = &paths.prefix {
        paths.wotc_config = find_user_config(prefix, WOTC_GAME_DIR);
        paths.vanilla_config = find_user_config(prefix, VANILLA_GAME_DIR);
    }

    Ok(paths)
}

fn dedup_paths(paths: &mut Vec<PathBuf>) {
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(path.clone()));
}

fn parse_library_folders(root: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) else {
        return Vec::new();
    };
    let Ok(re) = Regex::new(r#""path"\s+"((?:[^"\\]|\\.)*)""#) else {
        return Vec::new();
    };
    re.captures_iter(&text)
        .filter_map(|capture| capture.get(1))
        .map(|m| PathBuf::from(unescape_vdf(m.as_str())))
        .collect()
}

fn unescape_vdf(value: &str) -> String {
    value.replace("\\\\", "\\").replace("\\\"", "\"")
}

fn game_install_dir(library: &Path) -> PathBuf {
    let manifest = library
        .join("steamapps")
        .join(format!("appmanifest_{APP_ID}.acf"));
    if let Ok(text) = fs::read_to_string(manifest) {
        if let Ok(re) = Regex::new(r#""installdir"\s+"([^"]+)""#) {
            if let Some(capture) = re.captures(&text) {
                if let Some(dir) = capture.get(1) {
                    return library.join("steamapps/common").join(dir.as_str());
                }
            }
        }
    }
    library.join("steamapps/common/XCOM 2")
}

fn find_user_config(prefix: &Path, game_dir: &str) -> Option<PathBuf> {
    let users = prefix.join("drive_c/users");
    let mut fallback = None;
    if let Ok(entries) = fs::read_dir(&users) {
        for entry in entries.flatten() {
            let user_dir = entry.path();
            for documents in ["Documents", "My Documents", "My Documents BACKUP"] {
                let candidate = user_dir
                    .join(documents)
                    .join("my games")
                    .join(game_dir)
                    .join("XComGame/Config");
                if candidate.is_dir() {
                    return Some(candidate);
                }
                if fallback.is_none() {
                    fallback = Some(candidate);
                }
            }
        }
    }
    fallback
}

// ---------------------------------------------------------------------------
// AML settings import
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct AmlRoot {
    #[serde(rename = "Mods", default)]
    pub mods: AmlMods,
    #[serde(rename = "ModPaths", default)]
    pub mod_paths: Vec<String>,
    #[serde(rename = "GamePath", default)]
    pub game_path: String,
    #[serde(rename = "LastLaunchedWotC", default)]
    pub last_launched_wotc: bool,
    #[serde(rename = "ArgumentList", default)]
    pub argument_list: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct AmlMods {
    #[serde(rename = "Entries", default)]
    pub entries: BTreeMap<String, AmlGroup>,
}

#[derive(Debug, Default, Deserialize)]
pub struct AmlGroup {
    #[serde(rename = "Index", default)]
    pub index: i64,
    #[serde(rename = "Entries", default)]
    pub entries: Vec<AmlMod>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AmlMod {
    #[serde(rename = "ID", default)]
    pub id: String,
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
    #[serde(rename = "Author", default)]
    pub author: Option<String>,
    #[serde(rename = "Description", default)]
    pub description: Option<String>,
    #[serde(rename = "isActive", default)]
    pub is_active: bool,
    #[serde(rename = "Index", default)]
    pub index: i64,
    #[serde(rename = "Path", default)]
    pub path: Option<String>,
    #[serde(rename = "WorkshopID", default)]
    pub workshop_id: Option<u64>,
    #[serde(rename = "Source", default)]
    pub source: Option<i32>,
    #[serde(rename = "Note", default)]
    pub note: Option<String>,
    #[serde(rename = "Category", default)]
    pub category: Option<String>,
    #[serde(rename = "Size", default)]
    pub size: i64,
    #[serde(rename = "DateUpdated", default)]
    pub date_updated: Option<String>,
    #[serde(rename = "BuiltForWOTC", default)]
    pub built_for_wotc: bool,
    #[serde(rename = "Dependencies", default)]
    pub dependencies: Vec<i64>,
    #[serde(rename = "IgnoredDependencies", default)]
    pub ignored_dependencies: Vec<i64>,
}

impl AmlRoot {
    pub fn load(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("Could not read AML settings: {}", path.display()))?;
        let root: AmlRoot = serde_json::from_str(&text)
            .with_context(|| format!("Could not parse AML settings JSON: {}", path.display()))?;
        Ok(root)
    }

    pub fn all_mods(&self) -> Vec<AmlMod> {
        let mut mods = Vec::new();
        for (category, group) in &self.mods.entries {
            for entry in &group.entries {
                let mut entry = entry.clone();
                if entry.category.is_none() {
                    entry.category = Some(category.clone());
                }
                mods.push(entry);
            }
        }
        mods
    }

    pub fn active_sorted(&self) -> Vec<AmlMod> {
        let mut mods: Vec<AmlMod> = self
            .all_mods()
            .into_iter()
            .filter(|entry| entry.is_active)
            .collect();
        mods.sort_by(|a, b| {
            a.index
                .cmp(&b.index)
                .then_with(|| a.id.to_lowercase().cmp(&b.id.to_lowercase()))
        });
        mods
    }

    /// Merge AML's configured mods with Workshop folders that are not in
    /// settings.json yet. New subscriptions therefore show up on the next
    /// launch, before AML has been opened. Discovered mods start inactive and
    /// in the `Unsorted` category until the user enables/categorizes them.
    pub fn all_mods_with_workshop(&self, workshop_dir: Option<&Path>) -> Result<Vec<AmlMod>> {
        let mut mods = self.all_mods();
        let Some(workshop_dir) = workshop_dir else {
            return Ok(mods);
        };
        let index = scan_workshop_mods(workshop_dir)?;
        let known_ids: std::collections::HashSet<String> =
            mods.iter().map(|entry| entry.id.to_lowercase()).collect();
        let known_workshop: std::collections::HashSet<u64> = mods
            .iter()
            .filter_map(|entry| entry.workshop_id.filter(|id| *id > 0))
            .collect();

        let mut files: Vec<ModFile> = index.by_path.into_values().collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut seen_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        for file in files {
            let mod_dir = if file.path.is_dir() {
                file.path.clone()
            } else {
                file.path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| file.path.clone())
            };
            let Some(folder_id) = mod_dir
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.parse::<u64>().ok())
            else {
                continue;
            };
            if folder_id == 0
                || known_workshop.contains(&folder_id)
                || known_ids.contains(&file.internal_id.to_lowercase())
                || !seen_dirs.insert(mod_dir.clone())
            {
                continue;
            }

            mods.push(AmlMod {
                id: if file.internal_id.is_empty() {
                    folder_id.to_string()
                } else {
                    file.internal_id.clone()
                },
                name: file.title.clone(),
                author: file.author.clone(),
                description: file.description.clone(),
                is_active: false,
                index: -1,
                path: Some(mod_dir.to_string_lossy().to_string()),
                workshop_id: Some(folder_id),
                source: Some(1),
                note: None,
                category: Some("Unsorted".to_string()),
                size: directory_size(&mod_dir),
                date_updated: None,
                built_for_wotc: file.requires_xpack,
                dependencies: Vec::new(),
                ignored_dependencies: Vec::new(),
            });
        }
        Ok(mods)
    }
}

// ---------------------------------------------------------------------------
// .XComMod files
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
pub struct ModFile {
    pub path: PathBuf,
    pub internal_id: String,
    pub published_file_id: Option<u64>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub requires_xpack: bool,
}

#[derive(Debug, Default)]
pub struct ModIndex {
    pub by_workshop_id: HashMap<u64, ModFile>,
    pub by_internal_id: HashMap<String, ModFile>,
    pub by_path: HashMap<PathBuf, ModFile>,
}

impl ModIndex {
    fn insert(&mut self, file: ModFile) {
        if let Some(id) = file.published_file_id {
            self.by_workshop_id.insert(id, file.clone());
        }
        self.by_internal_id
            .insert(file.internal_id.to_lowercase(), file.clone());
        self.by_path.insert(file.path.clone(), file);
    }
}

pub fn scan_workshop_mods(workshop_dir: &Path) -> Result<ModIndex> {
    let mut index = ModIndex::default();
    let Ok(entries) = fs::read_dir(workshop_dir) else {
        return Ok(index);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string();
        let Ok(workshop_id) = dir_name.parse::<u64>() else {
            continue;
        };

        let mut mod_files: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = fs::read_dir(&path) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                let is_match = candidate
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("xcommod"));
                if is_match {
                    mod_files.push(candidate);
                }
            }
        }
        mod_files.sort();

        // Prefer the canonical <workshop-id>.XComMod, then anything else.
        let preferred = path.join(format!("{workshop_id}.XComMod"));
        let chosen = if mod_files.iter().any(|p| p == &preferred) {
            Some(preferred)
        } else {
            mod_files.into_iter().next()
        };

        if let Some(file) = chosen {
            if let Ok(mut parsed) = parse_xcomod(&file) {
                if parsed.published_file_id.unwrap_or(0) == 0 {
                    parsed.published_file_id = Some(workshop_id);
                }
                index.insert(parsed);
            }
        } else {
            // The directory exists but the mod file is missing; keep the
            // workshop id as a known path so active mods are not reported
            // missing just because the metadata is incomplete.
            let placeholder = ModFile {
                path: path.clone(),
                internal_id: dir_name.clone(),
                published_file_id: Some(workshop_id),
                ..Default::default()
            };
            index.insert(placeholder);
        }
    }

    Ok(index)
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

pub fn scan_local_mods(mod_dirs: &[PathBuf]) -> Result<ModIndex> {
    let mut index = ModIndex::default();
    for dir in mod_dirs {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let mut mod_files: Vec<PathBuf> = Vec::new();
            if let Ok(entries) = fs::read_dir(&path) {
                for entry in entries.flatten() {
                    let candidate = entry.path();
                    if candidate
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("xcommod"))
                    {
                        mod_files.push(candidate);
                    }
                }
            }
            mod_files.sort();
            if let Some(file) = mod_files.into_iter().next() {
                if let Ok(parsed) = parse_xcomod(&file) {
                    index.insert(parsed);
                }
            }
        }
    }
    Ok(index)
}

pub fn parse_xcomod(path: &Path) -> Result<ModFile> {
    let bytes =
        fs::read(path).with_context(|| format!("Could not read .XComMod: {}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let internal_id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string();

    let mut values: HashMap<String, String> = HashMap::new();
    let mut current_key: Option<String> = None;
    let valid_keys = [
        "publishedfileid",
        "title",
        "author",
        "category",
        "description",
        "tags",
        "contentimage",
        "requiresxpack",
    ];

    for (line_index, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line_index == 0 && line.trim().eq_ignore_ascii_case("[mod]") {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key_lower = key.trim().to_lowercase();
            if valid_keys.contains(&key_lower.as_str()) {
                values.insert(key_lower.clone(), value.to_string());
                current_key = Some(key_lower);
            } else if let Some(current) = &current_key {
                if let Some(existing) = values.get_mut(current) {
                    existing.push_str("\r\n");
                    existing.push_str(line);
                }
            }
        } else if let Some(current) = &current_key {
            if let Some(existing) = values.get_mut(current) {
                existing.push_str("\r\n");
                existing.push_str(line);
            }
        }
    }

    let published_file_id = values
        .get("publishedfileid")
        .and_then(|value| value.trim().parse::<u64>().ok());
    let requires_xpack = values
        .get("requiresxpack")
        .map(|value| {
            let value = value.trim().to_lowercase();
            value == "true" || value == "1"
        })
        .unwrap_or(false);

    Ok(ModFile {
        path: path.to_path_buf(),
        internal_id,
        published_file_id,
        title: values.get("title").cloned(),
        author: values.get("author").cloned(),
        description: values.get("description").cloned(),
        requires_xpack,
    })
}

// ---------------------------------------------------------------------------
// Windows -> Linux path mapping
// ---------------------------------------------------------------------------

pub fn map_windows_path(value: &str, prefix: Option<&Path>) -> Option<PathBuf> {
    let value = value.trim().trim_matches('"');
    if value.is_empty() {
        return None;
    }

    // Already a POSIX path.
    if value.starts_with('/') {
        return Some(PathBuf::from(value));
    }

    let bytes = value.as_bytes();
    if bytes.len() < 3 || bytes[1] != b':' || !(bytes[2] == b'\\' || bytes[2] == b'/') {
        return None;
    }

    let drive = (bytes[0] as char).to_ascii_lowercase();
    let rest = value[2..]
        .trim_start_matches(['\\', '/'])
        .replace('\\', "/");
    let prefix = prefix?;
    let link = prefix.join("dosdevices").join(format!("{drive}:"));

    let root = fs::read_link(&link).ok().or_else(|| {
        // Fallback for C: in case dosdevices is unavailable.
        if drive == 'c' {
            Some(PathBuf::from("drive_c"))
        } else if drive == 'z' {
            Some(PathBuf::from("/"))
        } else {
            None
        }
    })?;

    let root = if root.is_absolute() {
        root
    } else {
        prefix.join(root)
    };

    Some(root.join(rest))
}

// ---------------------------------------------------------------------------
// active mod list + INI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ActiveMod {
    pub internal_id: String,
    pub display_name: String,
    pub workshop_id: Option<u64>,
    pub path: Option<PathBuf>,
    pub aml_index: i64,
    pub category: String,
    pub missing: bool,
}

fn build_mod_index(paths: &SteamPaths) -> Result<ModIndex> {
    let mut index = ModIndex::default();
    if let Some(workshop_dir) = &paths.workshop_dir {
        index = scan_workshop_mods(workshop_dir)?;
    }
    if let Some(game_dir) = &paths.game_dir {
        let local_dirs = [
            game_dir.join("XComGame/Mods"),
            game_dir.join("XCom2-WarOfTheChosen/XComGame/Mods"),
        ];
        let local = scan_local_mods(&local_dirs)?;
        for (key, value) in local.by_internal_id {
            index.by_internal_id.insert(key, value);
        }
        for (key, value) in local.by_workshop_id {
            index.by_workshop_id.insert(key, value);
        }
        for (key, value) in local.by_path {
            index.by_path.insert(key, value);
        }
    }
    Ok(index)
}

fn resolve_mod_path(
    entry: &AmlMod,
    index: &ModIndex,
    paths: &SteamPaths,
) -> (Option<PathBuf>, bool) {
    let workshop_id = entry.workshop_id.filter(|id| *id > 0);

    if let Some(id) = workshop_id {
        if let Some(workshop_dir) = &paths.workshop_dir {
            let candidate = workshop_dir.join(id.to_string());
            if candidate.exists() {
                return (Some(candidate), false);
            }
        }
        if let Some(file) = index.by_workshop_id.get(&id) {
            let missing = !file.path.exists();
            return (Some(file.path.clone()), missing);
        }
    }

    if let Some(raw) = &entry.path {
        if let Some(mapped) = map_windows_path(raw, paths.prefix.as_deref()) {
            let missing = !mapped.exists();
            return (Some(mapped), missing);
        }
    }

    (None, true)
}

/// Build an active mod list in the exact order of `ids`.
///
/// Unknown IDs are preserved as missing placeholders. This matters when the
/// current XComModOptions.ini contains a manually installed mod that AML does
/// not know about: saving from the native UI must not silently drop it.
pub fn build_active_mods(
    settings: &AmlRoot,
    paths: &SteamPaths,
    ids: &[String],
) -> Result<Vec<ActiveMod>> {
    let index = build_mod_index(paths)?;
    let all = settings.all_mods_with_workshop(paths.workshop_dir.as_deref())?;
    let by_id: HashMap<&str, &AmlMod> =
        all.iter().map(|entry| (entry.id.as_str(), entry)).collect();

    let mut active = Vec::with_capacity(ids.len());
    for (position, id) in ids.iter().enumerate() {
        let Some(entry) = by_id.get(id.as_str()).copied() else {
            active.push(ActiveMod {
                internal_id: id.clone(),
                display_name: id.clone(),
                workshop_id: None,
                path: None,
                aml_index: position as i64,
                category: "Unknown".to_string(),
                missing: true,
            });
            continue;
        };
        let (path, missing) = resolve_mod_path(entry, &index, paths);
        let metadata = index.by_internal_id.get(&entry.id.to_lowercase());
        let display_name = entry
            .name
            .clone()
            .or_else(|| metadata.and_then(|file| file.title.clone()))
            .unwrap_or_else(|| entry.id.clone());

        active.push(ActiveMod {
            internal_id: entry.id.clone(),
            display_name,
            workshop_id: entry.workshop_id.filter(|id| *id > 0),
            path,
            aml_index: entry.index,
            category: entry
                .category
                .clone()
                .unwrap_or_else(|| "Unsorted".to_string()),
            missing,
        });
    }

    Ok(active)
}

pub fn resolve_active_mods(settings: &AmlRoot, paths: &SteamPaths) -> Result<Vec<ActiveMod>> {
    let ids: Vec<String> = settings
        .active_sorted()
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    build_active_mods(settings, paths, &ids)
}

pub fn render_mod_options(active: &[ActiveMod], ini_version: &str) -> String {
    let mut output = String::new();
    output.push_str("[Engine.XComModOptions]\r\n");
    for module in active {
        output.push_str("ActiveMods=");
        output.push_str(&module.internal_id);
        output.push_str("\r\n");
    }
    output.push_str("\r\n[IniVersion]\r\n0=");
    output.push_str(ini_version);
    output.push_str("\r\n\r\n");
    output
}

pub fn parse_active_mods(ini: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for line in ini.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("ActiveMods=") {
            let value = value.trim().trim_matches('"');
            if !value.is_empty() && seen.insert(value.to_string()) {
                result.push(value.to_string());
            }
        }
    }
    result
}

pub fn parse_ini_version(ini: &str) -> Option<String> {
    let mut in_version = false;
    for line in ini.lines() {
        let line = line.trim();
        if line.eq_ignore_ascii_case("[IniVersion]") {
            in_version = true;
            continue;
        }
        if in_version {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "0" {
                    return Some(value.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn default_mod_options_timestamp(game_dir: &Path, wotc: bool) -> Result<String> {
    let default_file = if wotc {
        game_dir
            .join(WOTC_DEFAULT_DIR)
            .join("XComGame/Config/DefaultModOptions.ini")
    } else {
        game_dir
            .join(VANILLA_DEFAULT_DIR)
            .join("Config/DefaultModOptions.ini")
    };

    let modified = fs::metadata(&default_file)
        .with_context(|| format!("Could not stat {}", default_file.display()))?
        .modified()
        .context("Could not read file mtime")?;
    let seconds = modified
        .duration_since(UNIX_EPOCH)
        .context("File mtime is before the Unix epoch")?
        .as_secs();
    Ok(format!("{seconds}.000000"))
}

// ---------------------------------------------------------------------------
// safe write
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct WriteResult {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
    pub active_count: usize,
    pub ini_version: String,
    pub unchanged: bool,
}

/// Write XComModOptions.ini with a backup and an atomic replace.
///
/// If the content is already identical, nothing is written and `unchanged`
/// is true.
pub fn write_mod_options(
    config_dir: &Path,
    content: &str,
    backup_root: &Path,
) -> Result<WriteResult> {
    let path = config_dir.join("XComModOptions.ini");
    let existing = match fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(err).with_context(|| format!("Could not read {}", path.display()));
        }
    };

    let active_count = content
        .lines()
        .filter(|line| line.starts_with("ActiveMods="))
        .count();
    let ini_version = parse_ini_version(content).unwrap_or_default();

    if existing.as_deref() == Some(content) {
        return Ok(WriteResult {
            path,
            backup: None,
            active_count,
            ini_version,
            unchanged: true,
        });
    }

    let mut backup = None;
    if existing.is_some() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("System time is before the Unix epoch")?
            .as_millis();
        let dir = backup_root.join(format!("{timestamp}-{}", std::process::id()));
        fs::create_dir_all(&dir)
            .with_context(|| format!("Could not create backup directory {}", dir.display()))?;
        let backup_file = dir.join("XComModOptions.ini");
        fs::copy(&path, &backup_file)
            .with_context(|| format!("Could not back up {}", path.display()))?;
        fs::write(
            dir.join("meta.txt"),
            format!(
                "date_ms={timestamp}\npath={}\nactive_count={active_count}\nini_version={ini_version}\n",
                path.display()
            ),
        )
        .with_context(|| format!("Could not write backup metadata in {}", dir.display()))?;
        backup = Some(backup_file);
    }

    fs::create_dir_all(config_dir)
        .with_context(|| format!("Could not create config directory {}", config_dir.display()))?;
    let tmp = config_dir.join(format!(".XComModOptions.ini.tmp-{}", std::process::id()));
    fs::write(&tmp, content.as_bytes())
        .with_context(|| format!("Could not write temporary file {}", tmp.display()))?;
    fs::rename(&tmp, &path).with_context(|| format!("Could not replace {}", path.display()))?;

    Ok(WriteResult {
        path,
        backup,
        active_count,
        ini_version,
        unchanged: false,
    })
}

// ---------------------------------------------------------------------------
// comparison
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct OrderDiff {
    pub identical: bool,
    pub first_mismatch: Option<usize>,
    pub missing_from_generated: Vec<String>,
    pub extra_in_generated: Vec<String>,
}

pub fn diff_order(current: &[String], generated: &[String]) -> OrderDiff {
    let current_set: HashSet<&String> = current.iter().collect();
    let generated_set: HashSet<&String> = generated.iter().collect();

    let first_mismatch = current
        .iter()
        .zip(generated.iter())
        .position(|(a, b)| a != b)
        .or_else(|| {
            if current.len() == generated.len() {
                None
            } else {
                Some(current.len().min(generated.len()))
            }
        });

    let missing_from_generated = current
        .iter()
        .filter(|id| !generated_set.contains(id))
        .cloned()
        .collect();
    let extra_in_generated = generated
        .iter()
        .filter(|id| !current_set.contains(id))
        .cloned()
        .collect();

    OrderDiff {
        identical: current == generated,
        first_mismatch,
        missing_from_generated,
        extra_in_generated,
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_aml_compatible_mod_options() {
        let active = vec![
            ActiveMod {
                internal_id: "ModA".to_string(),
                display_name: "Mod A".to_string(),
                workshop_id: None,
                path: None,
                aml_index: 0,
                category: "Unsorted".to_string(),
                missing: false,
            },
            ActiveMod {
                internal_id: "ModB".to_string(),
                display_name: "Mod B".to_string(),
                workshop_id: None,
                path: None,
                aml_index: 1,
                category: "Unsorted".to_string(),
                missing: false,
            },
        ];
        let output = render_mod_options(&active, "1784360590.000000");
        assert_eq!(
            output,
            "[Engine.XComModOptions]\r\nActiveMods=ModA\r\nActiveMods=ModB\r\n\r\n[IniVersion]\r\n0=1784360590.000000\r\n\r\n"
        );
    }

    #[test]
    fn parses_active_mods_and_versions() {
        let ini = "[Engine.XComModOptions]\r\nActiveMods=\"A\"\r\nActiveMods=B\r\nActiveMods=A\r\n\r\n[IniVersion]\r\n0=123.000000\r\n\r\n";
        assert_eq!(parse_active_mods(ini), vec!["A", "B"]);
        assert_eq!(parse_ini_version(ini), Some("123.000000".to_string()));
    }

    #[test]
    fn maps_z_drive_to_root() {
        let temp = std::env::temp_dir().join(format!("xcom2-core-test-{}", std::process::id()));
        let prefix = temp.join("pfx");
        fs::create_dir_all(prefix.join("dosdevices")).unwrap();
        std::os::unix::fs::symlink("/", prefix.join("dosdevices/z:")).unwrap();

        let mapped = map_windows_path("Z:\\tmp\\some file.txt", Some(&prefix)).unwrap();
        assert_eq!(mapped, PathBuf::from("/tmp/some file.txt"));

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn parses_xcomod_metadata() {
        let temp = std::env::temp_dir().join(format!("xcom2-xcomod-test-{}", std::process::id()));
        fs::create_dir_all(&temp).unwrap();
        let file = temp.join("TestMod.XComMod");
        fs::write(
            &file,
            "[mod]\r\npublishedfileid=12345\r\ntitle=Test Mod\r\ndescription=Hello\r\nworld\r\nrequiresxpack=true\r\n",
        )
        .unwrap();

        let parsed = parse_xcomod(&file).unwrap();
        assert_eq!(parsed.internal_id, "TestMod");
        assert_eq!(parsed.published_file_id, Some(12345));
        assert_eq!(parsed.title.as_deref(), Some("Test Mod"));
        assert_eq!(parsed.description.as_deref(), Some("Hello\r\nworld"));
        assert!(parsed.requires_xpack);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn discovers_new_workshop_mods() {
        let temp = std::env::temp_dir().join(format!(
            "xcom2-workshop-discovery-test-{}",
            std::process::id()
        ));
        let workshop = temp.join("workshop");
        fs::create_dir_all(workshop.join("123")).unwrap();
        fs::create_dir_all(workshop.join("999")).unwrap();
        fs::write(
            workshop.join("123/123.XComMod"),
            "[mod]\r\npublishedfileid=123\r\ntitle=Existing Mod\r\n",
        )
        .unwrap();
        fs::write(
            workshop.join("999/999.XComMod"),
            "[mod]\r\npublishedfileid=999\r\ntitle=New Mod\r\nrequiresxpack=true\r\n",
        )
        .unwrap();

        let mut settings = AmlRoot::default();
        let mut group = AmlGroup::default();
        group.entries.push(AmlMod {
            id: "Existing".to_string(),
            name: Some("Existing Mod".to_string()),
            workshop_id: Some(123),
            ..Default::default()
        });
        settings.mods.entries.insert("Test".to_string(), group);

        let mods = settings
            .all_mods_with_workshop(Some(&workshop))
            .expect("catalog");
        assert_eq!(mods.len(), 2);
        let new = mods
            .iter()
            .find(|entry| entry.workshop_id == Some(999))
            .expect("new workshop mod");
        assert_eq!(new.name.as_deref(), Some("New Mod"));
        assert!(!new.is_active);
        assert_eq!(new.category.as_deref(), Some("Unsorted"));
        assert!(new.built_for_wotc);
        assert_eq!(
            new.path.as_deref(),
            Some(workshop.join("999").to_str().unwrap())
        );

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn writes_mod_options_with_backup() {
        let temp = std::env::temp_dir().join(format!("xcom2-write-test-{}", std::process::id()));
        let config_dir = temp.join("config");
        let backup_root = temp.join("backups");
        fs::create_dir_all(&config_dir).unwrap();
        let path = config_dir.join("XComModOptions.ini");
        fs::write(&path, "old\r\n").unwrap();

        let new_content =
            "[Engine.XComModOptions]\r\nActiveMods=A\r\n\r\n[IniVersion]\r\n0=123.000000\r\n\r\n";
        let result = write_mod_options(&config_dir, new_content, &backup_root).unwrap();
        assert!(!result.unchanged);
        assert_eq!(result.active_count, 1);
        assert_eq!(result.ini_version, "123.000000");
        assert_eq!(fs::read_to_string(&path).unwrap(), new_content);
        let backup = result.backup.expect("backup should exist");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "old\r\n");

        let again = write_mod_options(&config_dir, new_content, &backup_root).unwrap();
        assert!(again.unchanged);
        assert!(again.backup.is_none());

        let _ = fs::remove_dir_all(&temp);
    }
}
