//! Native app state: mod categories and mod-list profiles.
//!
//! This is deliberately separate from the game config. Categories/profiles
//! are a UI convenience; the active mod list/order remains XComModOptions.ini.

use anyhow::{Context, Result, bail};
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileSource {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub imported_at: i64,
}

const MAX_PROFILE_REVISIONS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileRevision {
    pub id: i64,
    pub saved_at: i64,
    pub active_ids: Vec<String>,
    #[serde(default)]
    pub deactivated_mod_ids: Vec<String>,
    #[serde(default)]
    pub pending_workshop_ids: Vec<u64>,
    #[serde(default)]
    pub excluded_workshop_ids: Vec<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileRevisionView {
    pub id: i64,
    pub saved_at: i64,
    pub active_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileSyncResult {
    pub parent: String,
    pub active_count: usize,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub active_ids: Vec<String>,
    #[serde(default)]
    pub source: Option<ProfileSource>,
    #[serde(default)]
    pub collection_children: Vec<u64>,
    #[serde(default)]
    pub pending_workshop_ids: Vec<u64>,
    #[serde(default)]
    pub excluded_workshop_ids: Vec<u64>,
    #[serde(default)]
    pub deactivated_mod_ids: Vec<String>,
    #[serde(default)]
    pub cover_image: Option<String>,
    #[serde(default)]
    pub revisions: Vec<ProfileRevision>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub parent_updated: Option<i64>,
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
    pub description: Option<String>,
    pub active_ids: Vec<String>,
    pub count: usize,
    pub pending_count: usize,
    pub source_title: Option<String>,
    pub source_id: Option<u64>,
    pub source_url: Option<String>,
    pub deactivated_mod_ids: Vec<String>,
    pub cover_image: Option<String>,
    pub revision_count: usize,
    pub parent: Option<String>,
    pub parent_updated: Option<i64>,
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

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn revision_from_profile(profile: &Profile) -> ProfileRevision {
    ProfileRevision {
        id: 0,
        saved_at: profile.updated,
        active_ids: profile.active_ids.clone(),
        deactivated_mod_ids: profile.deactivated_mod_ids.clone(),
        pending_workshop_ids: profile.pending_workshop_ids.clone(),
        excluded_workshop_ids: profile.excluded_workshop_ids.clone(),
    }
}

fn revision_matches_profile(revision: &ProfileRevision, profile: &Profile) -> bool {
    revision.active_ids == profile.active_ids
        && revision.deactivated_mod_ids == profile.deactivated_mod_ids
        && revision.pending_workshop_ids == profile.pending_workshop_ids
        && revision.excluded_workshop_ids == profile.excluded_workshop_ids
}

fn push_profile_revision(profile: &mut Profile) {
    if profile
        .revisions
        .first()
        .is_some_and(|revision| revision_matches_profile(revision, profile))
    {
        return;
    }

    let mut revision = revision_from_profile(profile);
    let mut id = now_millis();
    while profile.revisions.iter().any(|existing| existing.id == id) {
        id += 1;
    }
    revision.id = id;
    profile.revisions.insert(0, revision);
    profile.revisions.truncate(MAX_PROFILE_REVISIONS);
}

pub fn push_profile_revision_by_name(state: &mut PersistedState, name: &str) -> Result<()> {
    let profile = state
        .profiles
        .iter_mut()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    push_profile_revision(profile);
    Ok(())
}

fn ensure_category(state: &mut PersistedState, name: &str) -> bool {
    if state
        .categories
        .iter()
        .any(|category| category.name == name)
    {
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
                state
                    .mod_categories
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
        let category = entry.category.unwrap_or_else(|| "Unsorted".to_string());
        ensure_category(state, &category);
        state.mod_categories.insert(entry.id, category);
        changed = true;
    }

    changed
}

pub fn category_views(
    state: &PersistedState,
    categories: &HashMap<String, usize>,
) -> Vec<CategoryView> {
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
            description: profile.description.clone(),
            active_ids: profile.active_ids.clone(),
            count: profile.active_ids.len(),
            pending_count: profile.pending_workshop_ids.len(),
            source_title: profile.source.as_ref().map(|source| source.title.clone()),
            source_id: profile.source.as_ref().map(|source| source.id),
            source_url: profile.source.as_ref().map(|source| source.url.clone()),
            deactivated_mod_ids: profile.deactivated_mod_ids.clone(),
            cover_image: profile.cover_image.clone(),
            revision_count: profile.revisions.len(),
            parent: profile.parent.clone(),
            parent_updated: profile.parent_updated,
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
    if state
        .categories
        .iter()
        .any(|category| category.name == name)
    {
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
    if let Some(profile) = state
        .profiles
        .iter_mut()
        .find(|profile| profile.name == name)
    {
        let active: HashSet<String> = active_ids.iter().cloned().collect();
        let mut next_deactivated = profile.deactivated_mod_ids.clone();
        next_deactivated.retain(|id| !active.contains(id));
        if profile.active_ids != active_ids || profile.deactivated_mod_ids != next_deactivated {
            push_profile_revision(profile);
            profile.active_ids = active_ids;
            profile.deactivated_mod_ids = next_deactivated;
            profile.updated = updated;
        }
    } else {
        state.profiles.push(Profile {
            name: name.to_string(),
            description: None,
            active_ids,
            source: None,
            collection_children: Vec::new(),
            pending_workshop_ids: Vec::new(),
            excluded_workshop_ids: Vec::new(),
            deactivated_mod_ids: Vec::new(),
            cover_image: None,
            revisions: Vec::new(),
            parent: None,
            parent_updated: None,
            updated,
        });
    }
    Ok(())
}

pub fn profile_history(state: &PersistedState, name: &str) -> Result<Vec<ProfileRevisionView>> {
    let profile = state
        .profiles
        .iter()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    Ok(profile
        .revisions
        .iter()
        .map(|revision| {
            let deactivated: HashSet<&str> = revision
                .deactivated_mod_ids
                .iter()
                .map(String::as_str)
                .collect();
            ProfileRevisionView {
                id: revision.id,
                saved_at: revision.saved_at,
                active_count: revision
                    .active_ids
                    .iter()
                    .filter(|id| !deactivated.contains(id.as_str()))
                    .count(),
            }
        })
        .collect())
}

pub fn get_profile_revision(
    state: &PersistedState,
    name: &str,
    revision_id: i64,
) -> Result<ProfileRevision> {
    let profile = state
        .profiles
        .iter()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    profile
        .revisions
        .iter()
        .find(|revision| revision.id == revision_id)
        .cloned()
        .with_context(|| format!("Profile revision not found: {revision_id}"))
}

pub fn restore_profile_revision(
    state: &mut PersistedState,
    name: &str,
    revision_id: i64,
) -> Result<Vec<String>> {
    let profile = state
        .profiles
        .iter_mut()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    let revision = profile
        .revisions
        .iter()
        .find(|revision| revision.id == revision_id)
        .cloned()
        .with_context(|| format!("Profile revision not found: {revision_id}"))?;

    if profile.active_ids == revision.active_ids
        && profile.deactivated_mod_ids == revision.deactivated_mod_ids
        && profile.pending_workshop_ids == revision.pending_workshop_ids
        && profile.excluded_workshop_ids == revision.excluded_workshop_ids
    {
        return Ok(profile.active_ids.clone());
    }

    push_profile_revision(profile);
    profile.active_ids = revision.active_ids;
    profile.deactivated_mod_ids = revision.deactivated_mod_ids;
    profile.pending_workshop_ids = revision.pending_workshop_ids;
    profile.excluded_workshop_ids = revision.excluded_workshop_ids;
    profile.updated = now();
    Ok(profile.active_ids.clone())
}

pub fn duplicate_profile(state: &mut PersistedState, source: &str, name: &str) -> Result<Profile> {
    let source = source.trim();
    let name = name.trim();
    if name.is_empty() {
        bail!("Profile name cannot be empty.");
    }
    if state.profiles.iter().any(|profile| profile.name == name) {
        bail!("Profile already exists: {name}");
    }
    let mut source_profile = state
        .profiles
        .iter()
        .find(|profile| profile.name == source)
        .with_context(|| format!("Profile not found: {source}"))?
        .clone();
    let source_updated = source_profile.updated;
    source_profile.name = name.to_string();
    source_profile.cover_image = None;
    source_profile.revisions = Vec::new();
    source_profile.parent = Some(source.to_string());
    source_profile.parent_updated = Some(source_updated);
    source_profile.updated = now();
    push_profile_revision(&mut source_profile);
    state.profiles.push(source_profile.clone());
    Ok(source_profile)
}

pub fn sync_profile_to_parent(state: &mut PersistedState, name: &str) -> Result<ProfileSyncResult> {
    let child_index = state
        .profiles
        .iter()
        .position(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    let parent_name = state.profiles[child_index]
        .parent
        .clone()
        .with_context(|| format!("Profile has no parent: {name}"))?;
    let parent_index = state
        .profiles
        .iter()
        .position(|profile| profile.name == parent_name)
        .with_context(|| format!("Parent profile not found: {parent_name}"))?;
    if parent_index == child_index {
        bail!("A profile cannot be its own parent.");
    }

    let child = &state.profiles[child_index];
    let child_active: HashSet<&str> = child.active_ids.iter().map(String::as_str).collect();
    let child_deactivated: HashSet<&str> = child
        .deactivated_mod_ids
        .iter()
        .map(String::as_str)
        .collect();
    let child_effective: HashSet<&str> = child_active
        .difference(&child_deactivated)
        .copied()
        .collect();
    let parent = &state.profiles[parent_index];
    let parent_active: HashSet<&str> = parent.active_ids.iter().map(String::as_str).collect();
    let parent_deactivated: HashSet<&str> = parent
        .deactivated_mod_ids
        .iter()
        .map(String::as_str)
        .collect();
    let parent_effective: HashSet<&str> = parent_active
        .difference(&parent_deactivated)
        .copied()
        .collect();
    let added = child_effective.difference(&parent_effective).count();
    let removed = parent_effective.difference(&child_effective).count();

    let child_active_ids = child.active_ids.clone();
    let child_deactivated_ids = child.deactivated_mod_ids.clone();
    let child_pending = child.pending_workshop_ids.clone();
    let child_excluded = child.excluded_workshop_ids.clone();

    let parent = &mut state.profiles[parent_index];
    let changed = parent.active_ids != child_active_ids
        || parent.deactivated_mod_ids != child_deactivated_ids
        || parent.pending_workshop_ids != child_pending
        || parent.excluded_workshop_ids != child_excluded;
    if changed {
        push_profile_revision(parent);
        parent.active_ids = child_active_ids;
        parent.deactivated_mod_ids = child_deactivated_ids;
        parent.pending_workshop_ids = child_pending;
        parent.excluded_workshop_ids = child_excluded;
        parent.updated = now();
    }
    let parent_updated = parent.updated;
    let active_count = parent.active_ids.len();

    state.profiles[child_index].parent_updated = Some(parent_updated);

    Ok(ProfileSyncResult {
        parent: parent_name,
        active_count,
        added,
        removed,
    })
}

/// Record or clear an explicit deactivation for a profile. A mod is only
/// recorded when it belongs to the profile's saved list; activating a mod that
/// was never in the profile is a no-op.
pub fn set_profile_mod_deactivated(
    state: &mut PersistedState,
    name: &str,
    mod_id: &str,
    deactivated: bool,
) -> Result<Vec<String>> {
    let profile = state
        .profiles
        .iter_mut()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    if deactivated {
        if profile.active_ids.iter().any(|id| id == mod_id)
            && !profile.deactivated_mod_ids.iter().any(|id| id == mod_id)
        {
            profile.deactivated_mod_ids.push(mod_id.to_string());
        }
    } else {
        profile.deactivated_mod_ids.retain(|id| id != mod_id);
    }
    profile.updated = now();
    Ok(profile.deactivated_mod_ids.clone())
}

/// Restore a deactivated mod to a profile: clear the explicit deactivation and
/// make sure it belongs to the profile's saved list again. Returns the updated
/// active list so the UI can keep the load order.
pub fn restore_profile_mod(
    state: &mut PersistedState,
    name: &str,
    mod_id: &str,
) -> Result<Vec<String>> {
    let profile = state
        .profiles
        .iter_mut()
        .find(|profile| profile.name == name)
        .with_context(|| format!("Profile not found: {name}"))?;
    profile.deactivated_mod_ids.retain(|id| id != mod_id);
    if !mod_id.is_empty() && !profile.active_ids.iter().any(|id| id == mod_id) {
        profile.active_ids.push(mod_id.to_string());
    }
    profile.updated = now();
    Ok(profile.active_ids.clone())
}

pub fn create_profile(state: &mut PersistedState, mut profile: Profile) -> Result<()> {
    let name = profile.name.trim().to_string();
    if name.is_empty() {
        bail!("Profile name cannot be empty.");
    }
    if state.profiles.iter().any(|existing| existing.name == name) {
        bail!("Profile already exists: {name}");
    }
    let mut seen = HashSet::new();
    profile.active_ids = profile
        .active_ids
        .into_iter()
        .filter(|id| !id.is_empty() && seen.insert(id.clone()))
        .collect();
    profile.name = name;
    profile.updated = now();
    state.profiles.push(profile);
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
            state
                .category_feedback
                .get("ModA")
                .map(|feedback| feedback.category.as_str()),
            Some("Weapons & Items")
        );

        rename_category(&mut state, "Weapons & Items", "Gear").unwrap();
        assert_eq!(
            state
                .category_feedback
                .get("ModA")
                .map(|feedback| feedback.category.as_str()),
            Some("Gear")
        );
        assert_eq!(
            state.mod_categories.get("ModA").map(String::as_str),
            Some("Gear")
        );

        delete_category(&mut state, "Gear").unwrap();
        assert!(!state.category_feedback.contains_key("ModA"));
        assert_eq!(
            state.mod_categories.get("ModA").map(String::as_str),
            Some("Unsorted")
        );
    }

    #[test]
    fn manual_feedback_can_be_cleared() {
        let mut state = PersistedState::default();
        set_mod_category_manual(&mut state, "ModA", "UI").unwrap();
        assert!(clear_mod_feedback(&mut state, "ModA"));
        assert!(!clear_mod_feedback(&mut state, "ModA"));
        assert!(state.category_feedback.is_empty());
    }

    #[test]
    fn collection_profiles_keep_source_and_pending_ids() {
        let mut state = PersistedState::default();
        create_profile(
            &mut state,
            Profile {
                name: "Star Wars".to_string(),
                description: Some("Collection description".to_string()),
                active_ids: vec!["ModA".to_string()],
                source: Some(ProfileSource {
                    kind: "steam_collection".to_string(),
                    id: 42,
                    title: "Star Wars Collection".to_string(),
                    url: "https://example.invalid/42".to_string(),
                    imported_at: 1,
                }),
                collection_children: vec![1, 2, 3],
                pending_workshop_ids: vec![2, 3],
                excluded_workshop_ids: Vec::new(),
                deactivated_mod_ids: Vec::new(),
                cover_image: None,
                revisions: Vec::new(),
                parent: None,
                parent_updated: None,
                updated: 0,
            },
        )
        .unwrap();
        assert!(
            create_profile(
                &mut state,
                Profile {
                    name: "Star Wars".to_string(),
                    description: None,
                    active_ids: Vec::new(),
                    source: None,
                    collection_children: Vec::new(),
                    pending_workshop_ids: Vec::new(),
                    excluded_workshop_ids: Vec::new(),
                    deactivated_mod_ids: Vec::new(),
                    cover_image: None,
                    revisions: Vec::new(),
                    parent: None,
                    parent_updated: None,
                    updated: 0,
                }
            )
            .is_err()
        );

        let views = profile_views(&state);
        assert_eq!(views.len(), 1);
        assert_eq!(
            views[0].source_title.as_deref(),
            Some("Star Wars Collection")
        );
        assert_eq!(views[0].source_id, Some(42));
        assert_eq!(views[0].pending_count, 2);
        assert_eq!(
            views[0].description.as_deref(),
            Some("Collection description")
        );

        save_profile(
            &mut state,
            "Star Wars",
            vec!["ModA".to_string(), "ModB".to_string()],
        )
        .unwrap();
        let profile = state
            .profiles
            .iter()
            .find(|p| p.name == "Star Wars")
            .unwrap();
        assert_eq!(profile.active_ids.len(), 2);
        assert_eq!(profile.source.as_ref().map(|s| s.id), Some(42));
        assert_eq!(profile.pending_workshop_ids, vec![2, 3]);
    }

    #[test]
    fn profile_deactivations_are_recorded_and_restored() {
        let mut state = PersistedState::default();
        create_profile(
            &mut state,
            Profile {
                name: "LWOTC".to_string(),
                description: None,
                active_ids: vec!["ModA".to_string(), "ModB".to_string()],
                source: None,
                collection_children: Vec::new(),
                pending_workshop_ids: Vec::new(),
                excluded_workshop_ids: Vec::new(),
                deactivated_mod_ids: Vec::new(),
                cover_image: None,
                revisions: Vec::new(),
                parent: None,
                parent_updated: None,
                updated: 0,
            },
        )
        .unwrap();

        let deactivated = set_profile_mod_deactivated(&mut state, "LWOTC", "ModB", true).unwrap();
        assert_eq!(deactivated, vec!["ModB".to_string()]);
        assert_eq!(
            profile_views(&state)[0].deactivated_mod_ids,
            vec!["ModB".to_string()]
        );

        // A mod that is not part of the profile is not recorded.
        let deactivated = set_profile_mod_deactivated(&mut state, "LWOTC", "ModC", true).unwrap();
        assert_eq!(deactivated, vec!["ModB".to_string()]);

        save_profile(&mut state, "LWOTC", vec!["ModA".to_string()]).unwrap();
        let profile = state.profiles.iter().find(|p| p.name == "LWOTC").unwrap();
        assert_eq!(profile.active_ids, vec!["ModA".to_string()]);
        assert_eq!(profile.deactivated_mod_ids, vec!["ModB".to_string()]);

        let active_ids = restore_profile_mod(&mut state, "LWOTC", "ModB").unwrap();
        assert_eq!(active_ids, vec!["ModA".to_string(), "ModB".to_string()]);
        let profile = state.profiles.iter().find(|p| p.name == "LWOTC").unwrap();
        assert!(profile.deactivated_mod_ids.is_empty());
    }

    fn test_profile(name: &str, ids: &[&str]) -> Profile {
        Profile {
            name: name.to_string(),
            description: None,
            active_ids: ids.iter().map(|id| id.to_string()).collect(),
            source: None,
            collection_children: Vec::new(),
            pending_workshop_ids: Vec::new(),
            excluded_workshop_ids: Vec::new(),
            deactivated_mod_ids: Vec::new(),
            cover_image: None,
            revisions: Vec::new(),
            parent: None,
            parent_updated: None,
            updated: 1,
        }
    }

    #[test]
    fn profile_history_records_previous_state_and_restore_is_undoable() {
        let mut state = PersistedState::default();
        create_profile(&mut state, test_profile("P", &["A", "B"])).unwrap();
        assert!(profile_history(&state, "P").unwrap().is_empty());

        save_profile(
            &mut state,
            "P",
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
        )
        .unwrap();
        let history = profile_history(&state, "P").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].active_count, 2);
        let revision = get_profile_revision(&state, "P", history[0].id).unwrap();
        assert_eq!(revision.active_ids, vec!["A", "B"]);

        restore_profile_revision(&mut state, "P", history[0].id).unwrap();
        assert_eq!(get_profile(&state, "P").unwrap(), vec!["A", "B"]);
        let history = profile_history(&state, "P").unwrap();
        assert_eq!(history.len(), 2);
        let latest = get_profile_revision(&state, "P", history[0].id).unwrap();
        assert_eq!(latest.active_ids, vec!["A", "B", "C"]);

        restore_profile_revision(&mut state, "P", history[0].id).unwrap();
        assert_eq!(get_profile(&state, "P").unwrap(), vec!["A", "B", "C"]);
    }

    #[test]
    fn duplicate_profile_links_parent_and_sync_updates_it() {
        let mut state = PersistedState::default();
        create_profile(&mut state, test_profile("Base", &["A", "B"])).unwrap();
        let source_updated = state
            .profiles
            .iter()
            .find(|profile| profile.name == "Base")
            .map(|profile| profile.updated)
            .unwrap();

        let child = duplicate_profile(&mut state, "Base", "Base test").unwrap();
        assert_eq!(child.parent.as_deref(), Some("Base"));
        assert_eq!(child.parent_updated, Some(source_updated));
        let child_view = profile_views(&state)
            .into_iter()
            .find(|view| view.name == "Base test")
            .unwrap();
        assert_eq!(child_view.revision_count, 1);

        save_profile(
            &mut state,
            "Base test",
            vec!["A".to_string(), "B".to_string(), "C".to_string()],
        )
        .unwrap();
        let result = sync_profile_to_parent(&mut state, "Base test").unwrap();
        assert_eq!(result.parent, "Base");
        assert_eq!(result.added, 1);
        assert_eq!(result.removed, 0);
        assert_eq!(get_profile(&state, "Base").unwrap(), vec!["A", "B", "C"]);

        let history = profile_history(&state, "Base").unwrap();
        assert_eq!(history.len(), 1);
        let revision = get_profile_revision(&state, "Base", history[0].id).unwrap();
        assert_eq!(revision.active_ids, vec!["A", "B"]);

        let parent_updated = state
            .profiles
            .iter()
            .find(|profile| profile.name == "Base")
            .map(|profile| profile.updated)
            .unwrap();
        let child = state
            .profiles
            .iter()
            .find(|profile| profile.name == "Base test")
            .unwrap();
        assert_eq!(child.parent_updated, Some(parent_updated));
    }

    #[test]
    fn profile_history_is_capped() {
        let mut state = PersistedState::default();
        create_profile(&mut state, test_profile("P", &["A"])).unwrap();
        for index in 0..25 {
            save_profile(&mut state, "P", vec![format!("Mod{index}")]).unwrap();
        }
        assert_eq!(profile_history(&state, "P").unwrap().len(), 20);
    }
}
