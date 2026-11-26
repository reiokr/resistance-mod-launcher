//! Read-only Steam Workshop metadata and update indicators.
//!
//! Metadata comes from Steam's public GetPublishedFileDetails endpoint;
//! local download timestamps come from appworkshop_268500.acf. No subscribe,
//! download, or write to Steam is performed here.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use xcom2_launcher_core::{discover_paths, AmlRoot, SteamPaths};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkshopDetails {
    pub workshop_id: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub time_created: Option<i64>,
    pub time_updated: Option<i64>,
    pub preview_url: Option<String>,
    pub file_size: Option<u64>,
    pub subscriptions: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkshopCache {
    pub items: HashMap<String, WorkshopDetails>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkshopRefresh {
    pub total: usize,
    pub fetched: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct WorkshopEnsure {
    pub requested: usize,
    pub fetched: usize,
    pub failed: usize,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope {
    response: ApiResponse,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(default)]
    publishedfiledetails: Vec<ApiItem>,
}

#[derive(Debug, Deserialize)]
struct ApiTag {
    #[serde(default)]
    tag: String,
}

#[derive(Debug, Deserialize)]
struct ApiItem {
    #[serde(default)]
    publishedfileid: String,
    #[serde(default)]
    result: Option<i32>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    tags: Vec<ApiTag>,
    #[serde(default)]
    time_created: Option<i64>,
    #[serde(default)]
    time_updated: Option<i64>,
    #[serde(default)]
    preview_url: Option<String>,
    #[serde(default)]
    file_size: Option<String>,
    #[serde(default)]
    subscriptions: Option<i64>,
}

pub fn cache_path() -> Result<PathBuf> {
    Ok(crate::app_data_dir()?.join("workshop_cache.json"))
}

pub fn load_cache() -> Result<WorkshopCache> {
    let path = cache_path()?;
    match fs::read_to_string(&path) {
        Ok(text) => {
            let mut cache: WorkshopCache = serde_json::from_str(&text).unwrap_or_default();
            for details in cache.items.values_mut() {
                let tags = std::mem::take(&mut details.tags);
                let mut seen = HashSet::new();
                details.tags = tags
                    .into_iter()
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty() && seen.insert(tag.to_lowercase()))
                    .collect();
            }
            Ok(cache)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(WorkshopCache::default()),
        Err(err) => Err(err).with_context(|| format!("Could not read {}", path.display())),
    }
}

fn save_cache(cache: &WorkshopCache) -> Result<()> {
    let path = cache_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string(cache)?;
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn acf_path(paths: &SteamPaths) -> Option<PathBuf> {
    if let Some(workshop_dir) = &paths.workshop_dir {
        // .../steamapps/workshop/content/268500 -> .../steamapps/workshop
        if let Some(root) = workshop_dir.parent().and_then(|p| p.parent()) {
            let candidate = root.join("appworkshop_268500.acf");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(library) = &paths.library {
        let candidate = library.join("steamapps/workshop/appworkshop_268500.acf");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Local last-updated timestamps from Steam's appworkshop_268500.acf.
pub fn local_times(paths: &SteamPaths) -> Result<HashMap<u64, i64>> {
    let mut result = HashMap::new();
    let Some(path) = acf_path(paths) else {
        return Ok(result);
    };
    let text = fs::read_to_string(&path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    let re = Regex::new(
        r#""(\d+)"\s*\{\s*"size"\s*"\d+"\s*"timeupdated"\s*"(\d+)"\s*"manifest"\s*"\d+"\s*\}"#,
    )?;
    for captures in re.captures_iter(&text) {
        let Ok(id) = captures[1].parse::<u64>() else {
            continue;
        };
        let Ok(timestamp) = captures[2].parse::<i64>() else {
            continue;
        };
        result.insert(id, timestamp);
    }
    Ok(result)
}

pub fn is_update_available(details: &WorkshopDetails, local_time: Option<i64>) -> bool {
    let local = local_time.unwrap_or(0);
    let remote = details.time_updated.unwrap_or(0);
    local > 0 && remote > local + 1
}

fn api_details(item: ApiItem) -> Option<WorkshopDetails> {
    if item.result.unwrap_or(1) == 0 {
        return None;
    }
    let workshop_id = item.publishedfileid.parse::<u64>().ok()?;
    let file_size = item
        .file_size
        .and_then(|value| value.parse::<u64>().ok());
    let mut seen = HashSet::new();
    let tags = item
        .tags
        .into_iter()
        .map(|tag| tag.tag.trim().to_string())
        .filter(|tag| !tag.is_empty() && seen.insert(tag.to_lowercase()))
        .collect();
    Some(WorkshopDetails {
        workshop_id,
        title: item.title,
        description: item.description,
        tags,
        time_created: item.time_created,
        time_updated: item.time_updated,
        preview_url: item.preview_url,
        file_size,
        subscriptions: item.subscriptions,
    })
}

fn fetch_details(agent: &ureq::Agent, ids: &[u64]) -> Result<Vec<WorkshopDetails>> {
    let mut body = format!("itemcount={}", ids.len());
    for (index, id) in ids.iter().enumerate() {
        body.push_str(&format!("&publishedfileids[{index}]={id}"));
    }

    let response = agent
        .post("https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/")
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string(&body)?;
    let text = response
        .into_string()
        .context("Steam Workshop returned invalid UTF-8")?;
    let envelope: ApiEnvelope =
        serde_json::from_str(&text).context("Could not parse Steam Workshop response")?;
    Ok(envelope
        .response
        .publishedfiledetails
        .into_iter()
        .filter_map(api_details)
        .collect())
}

/// Fetch and cache Workshop details for arbitrary IDs (used for missing
/// dependencies that are not part of AML's mod list). Best effort: callers
/// should treat failures as "name still unknown".
pub fn ensure_cached(ids: &[u64]) -> Result<WorkshopEnsure> {
    let mut requested: Vec<u64> = ids
        .iter()
        .copied()
        .filter(|id| *id > 0)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    requested.sort_unstable();
    if requested.is_empty() {
        return Ok(WorkshopEnsure::default());
    }

    let mut cache = load_cache()?;
    let mut missing: Vec<u64> = requested
        .iter()
        .copied()
        .filter(|id| {
            cache
                .items
                .get(&id.to_string())
                .and_then(|details| details.title.as_ref())
                .is_none()
        })
        .collect();
    if missing.is_empty() {
        return Ok(WorkshopEnsure {
            requested: requested.len(),
            fetched: 0,
            failed: 0,
        });
    }
    missing.sort_unstable();

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .build();
    let mut fetched = 0usize;
    let mut failed = 0usize;
    for chunk in missing.chunks(50) {
        match fetch_details(&agent, chunk) {
            Ok(details) => {
                let count = details.len();
                for details in details {
                    cache
                        .items
                        .insert(details.workshop_id.to_string(), details);
                    fetched += 1;
                }
                failed += chunk.len().saturating_sub(count);
            }
            Err(_) => failed += chunk.len(),
        }
        std::thread::sleep(Duration::from_millis(180));
    }
    if fetched > 0 {
        save_cache(&cache)?;
    }
    Ok(WorkshopEnsure {
        requested: requested.len(),
        fetched,
        failed,
    })
}

pub fn refresh() -> Result<WorkshopRefresh> {
    let paths = discover_paths()?;
    let settings_path = paths
        .settings
        .clone()
        .context("AML settings.json was not found")?;
    let settings = AmlRoot::load(&settings_path)?;

    let mut ids: Vec<u64> = settings
        .all_mods()
        .into_iter()
        .filter_map(|entry| entry.workshop_id.filter(|id| *id > 0))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    ids.sort_unstable();

    if ids.is_empty() {
        anyhow::bail!("No Workshop IDs found in AML settings.");
    }

    let mut cache = load_cache()?;
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build();
    let mut fetched = 0usize;
    let mut failed = 0usize;
    let mut errors = Vec::new();

    for chunk in ids.chunks(50) {
        match fetch_details(&agent, chunk) {
            Ok(details) => {
                let count = details.len();
                for details in details {
                    cache
                        .items
                        .insert(details.workshop_id.to_string(), details);
                    fetched += 1;
                }
                failed += chunk.len().saturating_sub(count);
            }
            Err(err) => {
                failed += chunk.len();
                if errors.len() < 5 {
                    errors.push(format!("Workshop batch failed: {err}"));
                }
            }
        }

        std::thread::sleep(Duration::from_millis(180));
    }

    save_cache(&cache)?;
    Ok(WorkshopRefresh {
        total: ids.len(),
        fetched,
        failed,
        errors,
    })
}
