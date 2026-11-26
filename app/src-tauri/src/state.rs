//! Native app state: mod categories and mod-list profiles.
//!
//! This is deliberately separate from the game config. Categories/profiles
//! are a UI convenience; the active mod list/order remains XComModOptions.ini.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use xcom2_launcher_core::AmlRoot;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub index: i64,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub active_ids: Vec<String>,
    pub updated: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryFeedback {
    pub category: String,
    pub updated: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PersistedState {
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub mod_categories: HashMap<String, String>,
    #[serde(default)]
    pub category_feedback: HashMap<String, CategoryFeedback>,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CategoryView {
    pub name: String,
    pub index: i64,
    pub collapsed: bool,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    pub name: String,
    pub count: usize,
    pub updated: i64,
}

fn state_path() -> Result<PathBuf> {
    Ok(crate::app_data_dir()?.join("state.json"))
}

pub fn load() -> Result<PersistedState> {
    let path = state_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_default()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(PersistedState::default()),
        Err(err) => Err(err).with_context(|| format!("Could not read {}", path.display())),
    }
}

pub fn save(state: &PersistedState) -> Result<()> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn next_index(state: &PersistedState) -> i64 {
    state
        .categories
        .iter()
        .map(|category| category.index)
        .max()
        .unwrap_or(-1)
        + 1
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn ensure_category(state: &mut PersistedState, name: &str) -> bool {
    if state.categories.iter().any(|category| category.name == name) {
        return false;
    }
    let index = next_index(state);
    state.categories.push(Category {
        name: name.to_string(),
        index,
        collapsed: false,
    });
    true
}

/// Import AML groups on first run and assign any new mods to a category.
pub fn ensure_from_aml(state: &mut PersistedState, settings: &AmlRoot) -> bool {
    let mut changed = false;

    if state.categories.is_empty() {
        for (name, group) in &settings.mods.entries {
            state.categories.push(Category {
                name: name.clone(),
                index: group.index,
                collapsed: false,
            });
            for entry in &group.entries {
                state.mod_categories
                    .entry(entry.id.clone())
                    .or_insert_with(|| name.clone());
            }
            changed = true;
        }
        if state.categories.is_empty() {
            state.categories.push(Category {
                name: "Unsorted".to_string(),
                index: 0,
                collapsed: false,
            });
            changed = true;
        }
    }

    if ensure_category(state, "Unsorted") {
        changed = true;
    }

    for entry in settings.all_mods() {
        if state.mod_categories.contains_key(&entry.id) {
            continue;
        }
        let category = entry
            .category
            .unwrap_or_else(|| "Unsorted".to_string());
        ensure_category(state, &category);
        state.mod_categories.insert(entry.id, category);
        changed = true;
    }

    changed
}

pub fn category_views(state: &PersistedState, categories: &HashMap<String, usize>) -> Vec<CategoryView> {
    let mut views: Vec<CategoryView> = state
        .categories
        .iter()
        .map(|category| CategoryView {
            name: category.name.clone(),
            index: category.index,
            collapsed: category.collapsed,
            count: categories.get(&category.name).copied().unwrap_or(0),
        })
        .collect();
    views.sort_by(|a, b| a.index.cmp(&b.index).then_with(|| a.name.cmp(&b.name)));
    views
}

pub fn profile_views(state: &PersistedState) -> Vec<ProfileView> {
    let mut views: Vec<ProfileView> = state
        .profiles
        .iter()
        .map(|profile| ProfileView {
            name: profile.name.clone(),
            count: profile.active_ids.len(),
            updated: profile.updated,
        })
        .collect();
    views.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    views
}

pub fn set_mod_category(state: &mut PersistedState, mod_id: &str, category: &str) -> Result<()> {
    let category = category.trim();
    if category.is_empty() {
        bail!("Category name cannot be empty.");
    }
    ensure_category(state, category);
    state
        .mod_categories
        .insert(mod_id.to_string(), category.to_string());
    Ok(())
}

/// Manual assignment from the UI: also records a labeled example used by the
/// category learner.
pub fn set_mod_category_manual(
    state: &mut PersistedState,
    mod_id: &str,
    category: &str,
) -> Result<()> {
    set_mod_category(state, mod_id, category)?;
    let category = category.trim().to_string();
    state.category_feedback.insert(
        mod_id.to_string(),
        CategoryFeedback {
            category,
            updated: now(),
        },
    );
    Ok(())
}

pub fn clear_category_feedback(state: &mut PersistedState) -> usize {
    let count = state.category_feedback.len();
    state.category_feedback.clear();
    count
}

pub fn clear_mod_feedback(state: &mut PersistedState, mod_id: &str) -> bool {
    state.category_feedback.remove(mod_id).is_some()
}

pub fn create_category(state: &mut PersistedState, name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Category name cannot be empty.");
    }
    if state.categories.iter().any(|category| category.name == name) {
        bail!("Category already exists: {name}");
    }
    let index = next_index(state);
    state.categories.push(Category {
        name: name.to_string(),
        index,
        collapsed: false,
    });
    Ok(())
}

pub fn rename_category(state: &mut PersistedState, old: &str, new: &str) -> Result<()> {
    let new = new.trim();
    if new.is_empty() {
        bail!("Category name cannot be empty.");
    }
    if old == new {
        return Ok(());
    }
    if state.categories.iter().any(|category| category.name == new) {
        bail!("Category already exists: {new}");
    }
    let Some(category) = state
        .categories
        .iter_mut()
        .find(|category| category.name == old)
    else {
        bail!("Category not found: {old}");
    };
    category.name = new.to_string();
    for value in state.mod_categories.values_mut() {
        if value == old {
            *value = new.to_string();
        }
    }
    for feedback in state.category_feedback.values_mut() {
        if feedback.category == old {
            feedback.category = new.to_string();
        }
    }
    if old == "Unsorted" {
        ensure_category(state, "Unsorted");
    }
    Ok(())
}

pub fn delete_category(state: &mut PersistedState, name: &str) -> Result<()> {
    if name == "Unsorted" {
        bail!("Unsorted cannot be deleted.");
    }
    let before = state.categories.len();
    state.categories.retain(|category| category.name != name);
    if state.categories.len() == before {
        bail!("Category not found: {name}");
    }
    for value in state.mod_categories.values_mut() {
        if value == name {
            *value = "Unsorted".to_string();
        }
    }
    state
        .category_feedback
        .retain(|_, feedback| feedback.category != name);
    ensure_category(state, "Unsorted");
    Ok(())
}

pub fn save_profile(state: &mut PersistedState, name: &str, active_ids: Vec<String>) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Profile name cannot be empty.");
    }
    let mut seen = HashSet::new();
    let active_ids: Vec<String> = active_ids
        .into_iter()
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect();
    let updated = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    if let Some(profile) = state.profiles.iter_mut().find(|profile| profile.name == name) {
        profile.active_ids = active_ids;
        profile.updated = updated;
    } else {
        state.profiles.push(Profile {
            name: name.to_string(),
            active_ids,
            updated,
        });
    }
    Ok(())
}

pub fn delete_profile(state: &mut PersistedState, name: &str) -> Result<()> {
    let before = state.profiles.len();
    state.profiles.retain(|profile| profile.name != name);
    if state.profiles.len() == before {
        bail!("Profile not found: {name}");
    }
    Ok(())
}

pub fn get_profile(state: &PersistedState, name: &str) -> Result<Vec<String>> {
    state
        .profiles
        .iter()
        .find(|profile| profile.name == name)
        .map(|profile| profile.active_ids.clone())
        .with_context(|| format!("Profile not found: {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_categories_are_recorded_and_renamed() {
        let mut state = PersistedState::default();
        set_mod_category_manual(&mut state, "ModA", "Weapons & Items").unwrap();
        assert_eq!(
            state.category_feedback.get("ModA").map(|feedback| feedback.category.as_str()),
            Some("Weapons & Items")
        );

        rename_category(&mut state, "Weapons & Items", "Gear").unwrap();
        assert_eq!(
            state.category_feedback.get("ModA").map(|feedback| feedback.category.as_str()),
            Some("Gear")
        );
        assert_eq!(state.mod_categories.get("ModA").map(String::as_str), Some("Gear"));

        delete_category(&mut state, "Gear").unwrap();
        assert!(!state.category_feedback.contains_key("ModA"));
        assert_eq!(state.mod_categories.get("ModA").map(String::as_str), Some("Unsorted"));
    }

    #[test]
    fn manual_feedback_can_be_cleared() {
        let mut state = PersistedState::default();
        set_mod_category_manual(&mut state, "ModA", "UI").unwrap();
        assert!(clear_mod_feedback(&mut state, "ModA"));
        assert!(!clear_mod_feedback(&mut state, "ModA"));
        assert!(state.category_feedback.is_empty());
    }
}
