//! Conflict, duplicate-ID and missing-dependency diagnostics.
//!
//! The rules mirror AML 1.6:
//! * class overrides come from mod Config/**/XComEngine.ini
//! * screen listeners come from mod Src/**/*.uc (ScreenClass = ...)
//! * a class is conflicted when >1 active mod overrides it, at least one
//!   override is a real class override, the mods differ, and the raw lines
//!   are not all identical
//! * dependencies must be installed and active (ignored dependencies skipped)

use anyhow::{Context, Result};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;
use xcom2_launcher_core::{
    discover_paths, parse_active_mods, AmlMod, AmlRoot, SteamPaths,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingDependency {
    pub workshop_id: u64,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub installed: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyIssue {
    pub mod_id: String,
    pub mod_name: String,
    pub missing: Vec<MissingDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideInfo {
    pub mod_id: String,
    pub mod_name: String,
    pub new_class: String,
    pub kind: String,
    pub text_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictIssue {
    pub base_class: String,
    pub overrides: Vec<OverrideInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModInfo {
    pub mod_id: String,
    pub name: String,
    pub workshop_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateIssue {
    pub id: String,
    pub mods: Vec<ModInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiagnosticsReport {
    pub scanned_at: i64,
    pub active_mods: usize,
    pub missing_dependency_count: usize,
    pub missing_dependency_links: usize,
    pub missing_dependencies: Vec<DependencyIssue>,
    pub conflicts: Vec<ConflictIssue>,
    pub conflict_mod_count: usize,
    pub duplicate_groups: Vec<DuplicateIssue>,
    pub duplicate_mod_count: usize,
}

#[derive(Debug, Clone)]
struct RawOverride {
    mod_id: String,
    mod_name: String,
    new_class: String,
    base_class: String,
    kind: String,
    text_line: String,
}

fn cache_path() -> Result<PathBuf> {
    Ok(crate::app_data_dir()?.join("diagnostics.json"))
}

pub fn load_cached() -> Result<Option<DiagnosticsReport>> {
    let path = cache_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text).ok()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("Could not read {}", path.display())),
    }
}

fn save_cache(report: &DiagnosticsReport) -> Result<()> {
    let path = cache_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(report)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn entry_path(entry: &AmlMod, paths: &SteamPaths) -> Option<PathBuf> {
    crate::mod_paths::entry_path(entry, paths)
}

fn read_text(path: &Path) -> String {
    fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .unwrap_or_default()
}

fn scan_overrides(entry: &AmlMod, paths: &SteamPaths) -> Vec<RawOverride> {
    let Some(mod_path) = entry_path(entry, paths) else {
        return Vec::new();
    };

    let class_re = Regex::new(
        r#"^[+]?ModClassOverrides=\(BaseGameClass="([^"]+)",ModClass="([^"]+)"\)"#,
    )
    .expect("class override regex");
    let whitespace_re = Regex::new(r"\s+").expect("whitespace regex");
    let screen_re =
        Regex::new(r"(?i)^\s*ScreenClass\s*=\s*(?:class')?([a-z_]+)").expect("screen regex");

    let mod_id = entry.id.clone();
    let mod_name = entry
        .name
        .clone()
        .unwrap_or_else(|| entry.id.clone());
    let mut result = Vec::new();

    let config = mod_path.join("Config");
    if config.is_dir() {
        for file in WalkDir::new(&config)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let path = file.path();
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("XComEngine.ini"))
            {
                continue;
            }
            let text = read_text(path);
            for line in text.lines() {
                let stripped = whitespace_re.replace_all(line, "");
                if let Some(captures) = class_re.captures(&stripped) {
                    result.push(RawOverride {
                        mod_id: mod_id.clone(),
                        mod_name: mod_name.clone(),
                        base_class: captures[1].to_string(),
                        new_class: captures[2].to_string(),
                        kind: "class".to_string(),
                        text_line: line.to_string(),
                    });
                }
            }
        }
    }

    let src = mod_path.join("Src");
    if src.is_dir() {
        for file in WalkDir::new(&src)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let path = file.path();
            if !path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("uc"))
            {
                continue;
            }
            let normalized = path.to_string_lossy().replace('\\', "/");
            if normalized.contains("/Src/XComGame") {
                continue;
            }
            let new_class = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_string();
            if new_class.is_empty() {
                continue;
            }
            let text = read_text(path);
            for line in text.lines() {
                if let Some(captures) = screen_re.captures(line) {
                    let base_class = captures[1].to_lowercase();
                    if base_class == "none" {
                        continue;
                    }
                    result.push(RawOverride {
                        mod_id: mod_id.clone(),
                        mod_name: mod_name.clone(),
                        base_class,
                        new_class: new_class.clone(),
                        kind: "screen".to_string(),
                        text_line: line.to_string(),
                    });
                }
            }
        }
    }

    result
}

fn detect_conflicts(overrides: Vec<RawOverride>) -> (Vec<ConflictIssue>, usize) {
    let mut by_base: HashMap<String, Vec<RawOverride>> = HashMap::new();
    for over in overrides {
        by_base
            .entry(over.base_class.to_lowercase())
            .or_default()
            .push(over);
    }

    let mut conflicts = Vec::new();
    let mut conflict_mods = HashSet::new();
    for (_, group) in by_base {
        if group.len() <= 1 {
            continue;
        }
        let mod_ids: HashSet<&str> = group.iter().map(|over| over.mod_id.as_str()).collect();
        let any_class = group.iter().any(|over| over.kind == "class");
        let text_lines: HashSet<&str> = group.iter().map(|over| over.text_line.as_str()).collect();
        if !any_class || mod_ids.len() <= 1 || text_lines.len() <= 1 {
            continue;
        }
        for over in &group {
            conflict_mods.insert(over.mod_id.clone());
        }
        let base_class = group[0].base_class.clone();
        let mut overrides: Vec<OverrideInfo> = group
            .into_iter()
            .map(|over| OverrideInfo {
                mod_id: over.mod_id,
                mod_name: over.mod_name,
                new_class: over.new_class,
                kind: over.kind,
                text_line: over.text_line,
            })
            .collect();
        overrides.sort_by(|a, b| a.mod_name.to_lowercase().cmp(&b.mod_name.to_lowercase()));
        conflicts.push(ConflictIssue {
            base_class,
            overrides,
        });
    }
    conflicts.sort_by(|a, b| a.base_class.to_lowercase().cmp(&b.base_class.to_lowercase()));
    (conflicts, conflict_mods.len())
}

pub fn scan() -> Result<DiagnosticsReport> {
    scan_with(None)
}

/// Scan with an explicit active list (the UI's unsaved state) or the saved
/// `XComModOptions.ini` when `active_ids` is `None`.
pub fn scan_with(active_ids_override: Option<Vec<String>>) -> Result<DiagnosticsReport> {
    let paths = discover_paths()?;
    let settings_path = paths
        .settings
        .clone()
        .context("AML settings.json was not found")?;
    let settings = AmlRoot::load(&settings_path)?;
    let all = settings.all_mods();

    let config_dir = paths
        .wotc_config
        .clone()
        .context("WotC config directory was not found")?;
    let config_path = config_dir.join("XComModOptions.ini");
    let active_ids = match active_ids_override {
        Some(ids) => ids,
        None => match fs::read_to_string(&config_path) {
            Ok(text) => parse_active_mods(&text),
            Err(_) => settings
                .active_sorted()
                .into_iter()
                .map(|entry| entry.id)
                .collect(),
        },
    };
    let active_set: HashSet<String> = active_ids.into_iter().collect();

    let all_by_workshop: HashMap<u64, &AmlMod> = all
        .iter()
        .filter_map(|entry| entry.workshop_id.filter(|id| *id > 0).map(|id| (id, entry)))
        .collect();
    let active_entries: Vec<&AmlMod> = all
        .iter()
        .filter(|entry| active_set.contains(&entry.id))
        .collect();

    let mut missing_dependencies = Vec::new();
    let mut missing_dependency_links = 0usize;
    for entry in &active_entries {
        let ignored: HashSet<i64> = entry.ignored_dependencies.iter().copied().collect();
        let mut missing = Vec::new();
        for dependency in &entry.dependencies {
            if *dependency <= 0 || ignored.contains(dependency) {
                continue;
            }
            let workshop_id = *dependency as u64;
            let installed = all_by_workshop.get(&workshop_id).copied();
            let active = installed
                .map(|mod_entry| active_set.contains(&mod_entry.id))
                .unwrap_or(false);
            if !active {
                missing.push(MissingDependency {
                    workshop_id,
                    title: installed.and_then(|mod_entry| mod_entry.name.clone()),
                    tags: Vec::new(),
                    installed: installed.is_some(),
                    active,
                });
            }
        }
        if !missing.is_empty() {
            missing_dependency_links += missing.len();
            missing_dependencies.push(DependencyIssue {
                mod_id: entry.id.clone(),
                mod_name: entry.name.clone().unwrap_or_else(|| entry.id.clone()),
                missing,
            });
        }
    }

    // Missing dependencies are not part of AML's mod list, so their names are
    // not in settings.json. Ask the public Workshop API for titles/tags once
    // and cache them; a network failure leaves the ID-only fallback.
    let unresolved: Vec<u64> = missing_dependencies
        .iter()
        .flat_map(|issue| {
            issue
                .missing
                .iter()
                .filter(|dependency| dependency.title.is_none())
                .map(|dependency| dependency.workshop_id)
        })
        .collect();
    if !unresolved.is_empty() {
        let _ = crate::workshop::ensure_cached(&unresolved);
    }
    let workshop_cache = crate::workshop::load_cache().unwrap_or_default();
    for issue in &mut missing_dependencies {
        for dependency in &mut issue.missing {
            if let Some(details) = workshop_cache
                .items
                .get(&dependency.workshop_id.to_string())
            {
                if dependency.title.is_none() {
                    dependency.title = details.title.clone();
                }
                dependency.tags = details.tags.clone();
            }
        }
    }

    missing_dependencies.sort_by(|a, b| a.mod_name.to_lowercase().cmp(&b.mod_name.to_lowercase()));

    let overrides: Vec<RawOverride> = active_entries
        .par_iter()
        .flat_map(|entry| scan_overrides(entry, &paths))
        .collect();

    let (conflicts, conflict_mod_count) = detect_conflicts(overrides);

    let mut duplicate_map: HashMap<String, Vec<ModInfo>> = HashMap::new();
    for entry in &all {
        duplicate_map
            .entry(entry.id.to_lowercase())
            .or_default()
            .push(ModInfo {
                mod_id: entry.id.clone(),
                name: entry.name.clone().unwrap_or_else(|| entry.id.clone()),
                workshop_id: entry.workshop_id.filter(|id| *id > 0),
            });
    }
    let mut duplicate_groups: Vec<DuplicateIssue> = duplicate_map
        .into_iter()
        .filter(|(_, mods)| mods.len() > 1)
        .map(|(id, mods)| DuplicateIssue { id, mods })
        .collect();
    duplicate_groups.sort_by(|a, b| a.id.cmp(&b.id));
    let duplicate_mod_count = duplicate_groups.iter().map(|group| group.mods.len()).sum();

    let scanned_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);

    let report = DiagnosticsReport {
        scanned_at,
        active_mods: active_entries.len(),
        missing_dependency_count: missing_dependencies.len(),
        missing_dependency_links,
        missing_dependencies,
        conflicts,
        conflict_mod_count,
        duplicate_groups,
        duplicate_mod_count,
    };

    save_cache(&report)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn over(mod_id: &str, base: &str, new: &str, kind: &str, line: &str) -> RawOverride {
        RawOverride {
            mod_id: mod_id.to_string(),
            mod_name: format!("Mod {mod_id}"),
            new_class: new.to_string(),
            base_class: base.to_string(),
            kind: kind.to_string(),
            text_line: line.to_string(),
        }
    }

    #[test]
    fn identical_override_lines_are_not_a_conflict() {
        let (conflicts, mods) = detect_conflicts(vec![
            over("A", "Base", "A.New", "class", "+ModClassOverrides=..."),
            over("B", "Base", "B.New", "class", "+ModClassOverrides=..."),
        ]);
        assert!(conflicts.is_empty());
        assert_eq!(mods, 0);
    }

    #[test]
    fn class_overrides_with_different_lines_conflict() {
        let (conflicts, mods) = detect_conflicts(vec![
            over("A", "Base", "A.New", "class", "line A"),
            over("B", "base", "B.New", "class", "line B"),
        ]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].overrides.len(), 2);
        assert_eq!(mods, 2);
    }

    #[test]
    fn screen_listener_only_is_not_a_conflict() {
        let (conflicts, mods) = detect_conflicts(vec![
            over("A", "Base", "A", "screen", "line A"),
            over("B", "Base", "B", "screen", "line B"),
        ]);
        assert!(conflicts.is_empty());
        assert_eq!(mods, 0);
    }

    #[test]
    fn class_and_screen_listener_conflict() {
        let (conflicts, mods) = detect_conflicts(vec![
            over("A", "Base", "A.New", "class", "line A"),
            over("B", "Base", "B", "screen", "line B"),
        ]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(mods, 2);
    }

    #[test]
    fn same_mod_id_is_not_a_conflict() {
        let (conflicts, mods) = detect_conflicts(vec![
            over("A", "Base", "A.New1", "class", "line A1"),
            over("A", "Base", "A.New2", "class", "line A2"),
        ]);
        assert!(conflicts.is_empty());
        assert_eq!(mods, 0);
    }
}
