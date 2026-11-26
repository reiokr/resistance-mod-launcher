//! Tauri backend for the native XCOM 2 mod launcher.
//!
//! It imports AML's settings.json for metadata, treats XComModOptions.ini as
//! the source of truth for the active list, and writes the config with a
//! backup + atomic replace.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use xcom2_launcher_core::{
    APP_ID, AmlMod, AmlRoot, SteamPaths, build_active_mods, default_mod_options_timestamp,
    discover_paths, map_windows_path, parse_active_mods, parse_ini_version, render_mod_options,
    write_mod_options,
};

mod auto_category;
mod category_learning;
mod config_editor;
mod diagnostics;
mod mod_paths;
mod readme;
mod state;
mod workshop;

#[derive(Debug, Serialize)]
struct ModView {
    id: String,
    name: String,
    author: Option<String>,
    category: String,
    workshop_id: Option<u64>,
    workshop_tags: Vec<String>,
    manual_category: bool,
    active: bool,
    order: Option<usize>,
    missing: bool,
    size: i64,
    date_updated: Option<String>,
    built_for_wotc: bool,
    path: Option<String>,
    update_available: bool,
}

#[derive(Debug, Serialize)]
struct Summary {
    total: usize,
    active: usize,
    missing: usize,
    unknown_active: usize,
    updates: usize,
    categories: usize,
    conflicts: usize,
    missing_dependencies: usize,
    args: Vec<String>,
    steam_running: bool,
    aml_running: bool,
    game_running: bool,
    config_exists: bool,
}

#[derive(Debug, Serialize)]
struct PathInfo {
    game_dir: Option<String>,
    prefix: Option<String>,
    workshop_dir: Option<String>,
    settings: Option<String>,
    config: Option<String>,
    backup_dir: String,
}

#[derive(Debug, Serialize)]
struct AppSnapshot {
    mods: Vec<ModView>,
    active_ids: Vec<String>,
    categories: Vec<state::CategoryView>,
    profiles: Vec<state::ProfileView>,
    diagnostics: Option<diagnostics::DiagnosticsReport>,
    summary: Summary,
    paths: PathInfo,
}

#[derive(Debug, Serialize)]
struct SaveResult {
    path: String,
    backup: Option<String>,
    active_count: usize,
    ini_version: String,
    unchanged: bool,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
struct LaunchResult {
    pid: u32,
    exe: String,
    args: Vec<String>,
    command: String,
    prelaunch: Option<String>,
    prelaunch_ok: bool,
}

#[derive(Debug, Serialize)]
struct ModDetails {
    id: String,
    workshop_id: Option<u64>,
    title: Option<String>,
    author: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
    time_created: Option<i64>,
    time_updated: Option<i64>,
    local_timeupdated: Option<i64>,
    update_available: bool,
    preview_url: Option<String>,
    subscriptions: Option<i64>,
}

fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

pub(crate) fn app_data_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".local/share/xcom2-launcher-linux"))
}

fn pgrep(pattern: &str) -> bool {
    Command::new("pgrep")
        .arg("-f")
        .arg(pattern)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn steam_running() -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg("steam")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn aml_running() -> bool {
    pgrep("launcher.exe")
}

fn game_running() -> bool {
    pgrep("XCom2.exe")
}

fn mod_path(entry: &AmlMod, paths: &SteamPaths) -> (Option<PathBuf>, bool) {
    if let Some(id) = entry.workshop_id.filter(|id| *id > 0) {
        if let Some(workshop) = &paths.workshop_dir {
            let candidate = workshop.join(id.to_string());
            if candidate.exists() {
                return (Some(candidate), false);
            }
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

fn load_snapshot() -> Result<AppSnapshot> {
    let paths = discover_paths()?;
    let settings_path = paths
        .settings
        .clone()
        .context("AML settings.json was not found (is AML installed in <game>/Launcher?)")?;
    let settings = AmlRoot::load(&settings_path)?;

    let mut app_state = state::load()?;
    if state::ensure_from_aml(&mut app_state, &settings) {
        state::save(&app_state)?;
    }

    let config_dir = paths
        .wotc_config
        .clone()
        .context("XCOM 2 War of the Chosen config directory was not found")?;
    let config_path = config_dir.join("XComModOptions.ini");

    let current_ids = match fs::read_to_string(&config_path) {
        Ok(text) => parse_active_mods(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => settings
            .active_sorted()
            .into_iter()
            .map(|entry| entry.id)
            .collect(),
        Err(err) => {
            return Err(err).with_context(|| format!("Could not read {}", config_path.display()));
        }
    };

    let order_map: HashMap<String, usize> = current_ids
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect();

    let mut all = settings.all_mods_with_workshop(paths.workshop_dir.as_deref())?;
    let cache = workshop::load_cache().unwrap_or_default();
    for entry in &mut all {
        if entry.name.as_deref().map_or(true, |name| name == entry.id) {
            if let Some(title) = entry
                .workshop_id
                .and_then(|id| cache.items.get(&id.to_string()))
                .and_then(|details| details.title.clone())
            {
                entry.name = Some(title);
            }
        }
    }
    let local_times = workshop::local_times(&paths).unwrap_or_default();
    let mut mods = Vec::with_capacity(all.len());
    for entry in &all {
        let (path, missing) = mod_path(entry, &paths);
        let order = order_map.get(&entry.id).copied();
        let workshop_id = entry.workshop_id.filter(|id| *id > 0);
        let update_available = match workshop_id {
            Some(id) => cache
                .items
                .get(&id.to_string())
                .map(|details| {
                    workshop::is_update_available(details, local_times.get(&id).copied())
                })
                .unwrap_or(false),
            None => false,
        };
        let workshop_tags = workshop_id
            .and_then(|id| cache.items.get(&id.to_string()))
            .map(|details| details.tags.clone())
            .unwrap_or_default();
        let category = app_state
            .mod_categories
            .get(&entry.id)
            .cloned()
            .or_else(|| entry.category.clone())
            .unwrap_or_else(|| "Unsorted".to_string());
        mods.push(ModView {
            id: entry.id.clone(),
            name: entry.name.clone().unwrap_or_else(|| entry.id.clone()),
            author: entry.author.clone(),
            category,
            workshop_id,
            workshop_tags,
            manual_category: app_state.category_feedback.contains_key(&entry.id),
            active: order.is_some(),
            order,
            missing,
            size: entry.size,
            date_updated: entry.date_updated.clone(),
            built_for_wotc: entry.built_for_wotc,
            path: path.map(|path| path.display().to_string()),
            update_available,
        });
    }
    mods.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });

    let known: HashSet<&str> = all.iter().map(|entry| entry.id.as_str()).collect();
    let unknown_active = current_ids
        .iter()
        .filter(|id| !known.contains(id.as_str()))
        .count();
    let missing = mods
        .iter()
        .filter(|entry| entry.active && entry.missing)
        .count();
    let updates = mods.iter().filter(|entry| entry.update_available).count();
    let mut category_counts: HashMap<String, usize> = HashMap::new();
    for entry in &mods {
        *category_counts.entry(entry.category.clone()).or_insert(0) += 1;
    }
    let categories = state::category_views(&app_state, &category_counts);
    let profiles = state::profile_views(&app_state);
    let diagnostics = diagnostics::load_cached().ok().flatten();
    let conflict_count = diagnostics
        .as_ref()
        .map(|report| report.conflicts.len())
        .unwrap_or(0);
    let missing_dependency_links = diagnostics
        .as_ref()
        .map(|report| report.missing_dependency_links)
        .unwrap_or(0);
    let args = if settings.argument_list.is_empty() {
        vec!["-review".to_string(), "-noRedScreens".to_string()]
    } else {
        settings.argument_list.clone()
    };
    let backup_dir = app_data_dir()?.join("backups");
    let active_count = current_ids.len();

    Ok(AppSnapshot {
        mods,
        active_ids: current_ids,
        categories,
        profiles,
        diagnostics,
        summary: Summary {
            total: all.len(),
            active: active_count,
            missing,
            unknown_active,
            updates,
            categories: category_counts.len(),
            conflicts: conflict_count,
            missing_dependencies: missing_dependency_links,
            args,
            steam_running: steam_running(),
            aml_running: aml_running(),
            game_running: game_running(),
            config_exists: config_path.exists(),
        },
        paths: PathInfo {
            game_dir: paths
                .game_dir
                .as_ref()
                .map(|path| path.display().to_string()),
            prefix: paths.prefix.as_ref().map(|path| path.display().to_string()),
            workshop_dir: paths
                .workshop_dir
                .as_ref()
                .map(|path| path.display().to_string()),
            settings: Some(settings_path.display().to_string()),
            config: Some(config_path.display().to_string()),
            backup_dir: backup_dir.display().to_string(),
        },
    })
}

fn save_state_impl(active_ids: Vec<String>) -> Result<SaveResult> {
    if aml_running() {
        bail!("AML is still open. Close AML before saving — it holds settings.json.");
    }
    if game_running() {
        bail!("XCOM 2 is running. Close the game before saving.");
    }

    let mut seen = HashSet::new();
    let active_ids: Vec<String> = active_ids
        .into_iter()
        .filter(|id| !id.trim().is_empty() && seen.insert(id.clone()))
        .collect();

    let paths = discover_paths()?;
    let settings_path = paths
        .settings
        .clone()
        .context("AML settings.json was not found")?;
    let settings = AmlRoot::load(&settings_path)?;
    let active = build_active_mods(&settings, &paths, &active_ids)?;

    let game_dir = paths
        .game_dir
        .as_deref()
        .context("XCOM 2 game directory was not found")?;
    let config_dir = paths
        .wotc_config
        .clone()
        .context("WotC config directory was not found")?;
    let config_path = config_dir.join("XComModOptions.ini");
    let current_version = fs::read_to_string(&config_path)
        .ok()
        .and_then(|text| parse_ini_version(&text));

    let timestamp = match default_mod_options_timestamp(game_dir, true) {
        Ok(timestamp) => timestamp,
        Err(_) => current_version
            .clone()
            .unwrap_or_else(|| "0.000000".to_string()),
    };

    let content = render_mod_options(&active, &timestamp);
    let backup_root = app_data_dir()?.join("backups");
    let written = write_mod_options(&config_dir, &content, &backup_root)?;

    let mut warnings = Vec::new();
    let missing_count = active.iter().filter(|entry| entry.missing).count();
    if missing_count > 0 {
        warnings.push(format!("{missing_count} active mod folders are missing"));
    }

    let engine_path = config_dir.join("XComEngine.ini");
    if let Ok(engine) = fs::read_to_string(&engine_path) {
        if !engine
            .lines()
            .any(|line| line.trim_start().starts_with("ModRootDirs="))
        {
            warnings.push("XComEngine.ini is missing ModRootDirs — mods may not load".to_string());
        }
    }

    Ok(SaveResult {
        path: written.path.display().to_string(),
        backup: written.backup.map(|path| path.display().to_string()),
        active_count: written.active_count,
        ini_version: written.ini_version,
        unchanged: written.unchanged,
        warnings,
    })
}

fn launch_game_impl() -> Result<LaunchResult> {
    if game_running() {
        bail!("XCOM 2 is already running.");
    }
    if aml_running() {
        bail!("AML is open. Close AML before launching the game.");
    }
    if !steam_running() {
        bail!("Steam is not running. Start Steam and sign in.");
    }
    let paths = discover_paths()?;
    let settings_path = paths
        .settings
        .clone()
        .context("AML settings.json was not found")?;
    let settings = AmlRoot::load(&settings_path)?;
    let game_dir = paths
        .game_dir
        .as_deref()
        .context("XCOM 2 game directory was not found")?;

    let wotc_exe = game_dir.join("XCom2-WarOfTheChosen/Binaries/Win64/XCom2.exe");
    let (exe, cwd) = if wotc_exe.is_file() {
        (wotc_exe, game_dir.join("XCom2-WarOfTheChosen"))
    } else {
        let vanilla = game_dir.join("Binaries/Win64/XCom2.exe");
        if !vanilla.is_file() {
            bail!("XCOM 2 executable not found under {}", game_dir.display());
        }
        (vanilla, game_dir.to_path_buf())
    };

    let args = if settings.argument_list.is_empty() {
        vec!["-review".to_string(), "-noRedScreens".to_string()]
    } else {
        settings.argument_list.clone()
    };

    // Optional prelaunch hook. The user's Steam launch options run
    // reapply-xcom2-fixes.sh via xcom2-preflight; when the native app starts
    // the game directly, we run the same hook here instead.
    let prelaunch_path = home_dir()?.join(".config/xcom2-launcher-linux/prelaunch");
    let mut prelaunch = None;
    let mut prelaunch_ok = true;
    if prelaunch_path.is_file() {
        match Command::new(&prelaunch_path).output() {
            Ok(output) => {
                if !output.status.success() {
                    prelaunch_ok = false;
                }
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let text = [stdout, stderr]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                if !text.is_empty() {
                    prelaunch = Some(text);
                }
            }
            Err(err) => {
                prelaunch_ok = false;
                prelaunch = Some(format!("prelaunch failed: {err}"));
            }
        }
    }

    // Launch the game from a Windows cmd/batch, not directly from Linux.
    //
    // AML works because its game child inherits SteamAppId/SteamGameId from
    // the launcher's Steamworks environment. A bare protontricks-launch of
    // XCom2.exe lacks those, and the big mod pack crashes at startup. The batch
    // recreates AML's environment and then runs the WotC exe.
    let data_dir = app_data_dir()?;
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("Could not create {}", data_dir.display()))?;
    let batch_path = data_dir.join("start-wotc.bat");

    let exe_win = windows_z_path(&exe);
    let cwd_win = windows_z_path(&cwd);
    let mut batch = String::new();
    batch.push_str("@echo off\r\n");
    batch.push_str("set SteamAppId=268500\r\n");
    batch.push_str("set SteamGameId=268500\r\n");
    batch.push_str("set SteamOverlayGameId=268500\r\n");
    batch.push_str(&format!("cd /d \"{}\"\r\n", cwd_win));
    batch.push_str(&format!("\"{}\"", exe_win));
    for arg in &args {
        batch.push(' ');
        batch.push_str(&quote_batch_arg(arg));
    }
    batch.push_str("\r\n");

    fs::write(&batch_path, batch.as_bytes())
        .with_context(|| format!("Could not write {}", batch_path.display()))?;

    let cmd_exe = paths
        .prefix
        .as_ref()
        .context("Proton prefix was not found")?
        .join("drive_c/windows/system32/cmd.exe");
    if !cmd_exe.exists() {
        bail!("Windows cmd.exe was not found in the Proton prefix.");
    }

    let batch_win = windows_z_path(&batch_path);
    let mut command = Command::new("protontricks-launch");
    command
        .arg("--no-term")
        .arg("--appid")
        .arg(APP_ID.to_string())
        .arg(&cmd_exe)
        .arg("/c")
        .arg(&batch_win);

    let child = command.spawn().with_context(|| {
        "protontricks-launch did not start. Is protontricks installed?".to_string()
    })?;

    Ok(LaunchResult {
        pid: child.id(),
        exe: exe.display().to_string(),
        args: args.clone(),
        command: format!(
            "protontricks-launch --no-term --appid {APP_ID} {} /c {}",
            cmd_exe.display(),
            batch_win
        ),
        prelaunch,
        prelaunch_ok,
    })
}

fn windows_z_path(path: &std::path::Path) -> String {
    let text = path.to_string_lossy().replace('/', "\\");
    if text.starts_with('\\') {
        format!("Z:{text}")
    } else {
        format!("Z:\\{text}")
    }
}

fn quote_batch_arg(value: &str) -> String {
    if value.contains(' ') || value.contains('\t') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[tauri::command]
fn get_state() -> Result<AppSnapshot, String> {
    load_snapshot().map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
fn save_state(active_ids: Vec<String>) -> Result<SaveResult, String> {
    save_state_impl(active_ids).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn launch_game() -> Result<LaunchResult, String> {
    launch_game_impl().map_err(|err| format!("{err:#}"))
}

#[tauri::command]
async fn refresh_workshop() -> Result<workshop::WorkshopRefresh, String> {
    tauri::async_runtime::spawn_blocking(workshop::refresh)
        .await
        .map_err(|err| format!("Workshop refresh task failed: {err}"))?
        .map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
async fn scan_diagnostics(
    active_ids: Option<Vec<String>>,
) -> Result<diagnostics::DiagnosticsReport, String> {
    tauri::async_runtime::spawn_blocking(move || diagnostics::scan_with(active_ids))
        .await
        .map_err(|err| format!("Diagnostics task failed: {err}"))?
        .map_err(|err| format!("{err:#}"))
}

fn get_mod_details_impl(id: &str) -> Result<ModDetails> {
    let paths = discover_paths()?;
    let settings_path = paths
        .settings
        .clone()
        .context("AML settings.json was not found")?;
    let settings = AmlRoot::load(&settings_path)?;
    let entry = settings
        .all_mods_with_workshop(paths.workshop_dir.as_deref())?
        .into_iter()
        .find(|entry| entry.id == id)
        .with_context(|| format!("Mod not found: {id}"))?;

    let cache = workshop::load_cache()?;
    let local_times = workshop::local_times(&paths)?;
    let workshop_id = entry.workshop_id.filter(|value| *value > 0);
    let details = workshop_id
        .and_then(|id| cache.items.get(&id.to_string()))
        .cloned()
        .unwrap_or_default();
    let local_time = workshop_id.and_then(|id| local_times.get(&id).copied());
    let update_available = workshop_id
        .map(|_| workshop::is_update_available(&details, local_time))
        .unwrap_or(false);

    Ok(ModDetails {
        id: entry.id,
        workshop_id,
        title: entry.name.or(details.title),
        author: entry.author,
        description: details.description.or(entry.description),
        tags: details.tags,
        time_created: details.time_created,
        time_updated: details.time_updated,
        local_timeupdated: local_time,
        update_available,
        preview_url: details.preview_url,
        subscriptions: details.subscriptions,
    })
}

#[tauri::command]
fn get_mod_details(id: String) -> Result<ModDetails, String> {
    get_mod_details_impl(&id).map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
fn get_mod_readme(mod_id: String) -> Result<readme::ReadmeContent, String> {
    readme::read(&mod_id).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn open_workshop(id: u64) -> Result<(), String> {
    let url = format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}");
    if Command::new("xdg-open").arg(&url).spawn().is_ok() {
        return Ok(());
    }
    Command::new("steam")
        .arg(format!("steam://url/CommunityFilePage/{id}"))
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Could not open Workshop page: {err}"))
}

#[tauri::command(rename_all = "snake_case")]
fn set_mod_category(mod_id: String, category: String) -> Result<(), String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    state::set_mod_category_manual(&mut app_state, &mod_id, &category)
        .map_err(|err| format!("{err:#}"))?;
    state::save(&app_state).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn create_category(name: String) -> Result<(), String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    state::create_category(&mut app_state, &name).map_err(|err| format!("{err:#}"))?;
    state::save(&app_state).map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
fn rename_category(old: String, new: String) -> Result<(), String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    state::rename_category(&mut app_state, &old, &new).map_err(|err| format!("{err:#}"))?;
    state::save(&app_state).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn delete_category(name: String) -> Result<(), String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    state::delete_category(&mut app_state, &name).map_err(|err| format!("{err:#}"))?;
    state::save(&app_state).map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
fn save_profile(name: String, active_ids: Vec<String>) -> Result<(), String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    state::save_profile(&mut app_state, &name, active_ids).map_err(|err| format!("{err:#}"))?;
    state::save(&app_state).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn delete_profile(name: String) -> Result<(), String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    state::delete_profile(&mut app_state, &name).map_err(|err| format!("{err:#}"))?;
    state::save(&app_state).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn get_profile(name: String) -> Result<Vec<String>, String> {
    let app_state = state::load().map_err(|err| format!("{err:#}"))?;
    state::get_profile(&app_state, &name).map_err(|err| format!("{err:#}"))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command(rename_all = "snake_case")]
async fn preview_steam_collection(
    collection_id: u64,
) -> Result<workshop::CollectionPreview, String> {
    tauri::async_runtime::spawn_blocking(move || workshop::preview_collection(collection_id))
        .await
        .map_err(|err| format!("Collection task failed: {err}"))?
        .map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
fn create_empty_profile(name: String, description: Option<String>) -> Result<String, String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    let profile = state::Profile {
        name: name.clone(),
        description: description.filter(|value| !value.trim().is_empty()),
        active_ids: Vec::new(),
        source: None,
        collection_children: Vec::new(),
        pending_workshop_ids: Vec::new(),
        excluded_workshop_ids: Vec::new(),
        updated: now_unix(),
    };
    state::create_profile(&mut app_state, profile).map_err(|err| format!("{err:#}"))?;
    state::save(&app_state).map_err(|err| format!("{err:#}"))?;
    Ok(name)
}

fn strip_bbcode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        if ch == '[' {
            in_tag = true;
        } else if ch == ']' && in_tag {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    out.replace('\r', "").trim().to_string()
}

fn create_profile_from_collection_impl(
    name: String,
    description: Option<String>,
    collection_id: u64,
    selected_mod_ids: Vec<String>,
    include_pending: bool,
) -> Result<String> {
    let preview = workshop::preview_collection(collection_id)?;
    let installed: HashMap<&str, &workshop::CollectionItem> = preview
        .items
        .iter()
        .filter(|item| item.installed)
        .filter_map(|item| item.mod_id.as_deref().map(|id| (id, item)))
        .collect();
    let selected_set: HashSet<&str> = selected_mod_ids.iter().map(String::as_str).collect();
    let mut active_ids = Vec::new();
    let mut seen = HashSet::new();
    for id in &selected_mod_ids {
        if installed.contains_key(id.as_str()) && seen.insert(id.clone()) {
            active_ids.push(id.clone());
        }
    }
    let excluded_workshop_ids = preview
        .items
        .iter()
        .filter(|item| item.status == "installed")
        .filter(|item| {
            item.mod_id
                .as_deref()
                .is_some_and(|id| !selected_set.contains(id))
        })
        .map(|item| item.workshop_id)
        .collect();
    let pending_workshop_ids = if include_pending {
        preview
            .items
            .iter()
            .filter(|item| item.status == "missing")
            .map(|item| item.workshop_id)
            .collect()
    } else {
        Vec::new()
    };
    let collection_children = preview
        .items
        .iter()
        .filter(|item| item.status != "other_game")
        .map(|item| item.workshop_id)
        .collect();
    let name = if name.trim().is_empty() {
        preview.title.clone()
    } else {
        name.trim().to_string()
    };
    let description = description
        .map(|value| strip_bbcode(&value))
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let value = strip_bbcode(&preview.description);
            if value.is_empty() { None } else { Some(value) }
        });
    let source = state::ProfileSource {
        kind: "steam_collection".to_string(),
        id: collection_id,
        title: preview.title.clone(),
        url: preview.url.clone(),
        imported_at: now_unix(),
    };
    let mut app_state = state::load()?;
    state::create_profile(
        &mut app_state,
        state::Profile {
            name: name.clone(),
            description,
            active_ids,
            source: Some(source),
            collection_children,
            pending_workshop_ids,
            excluded_workshop_ids,
            updated: now_unix(),
        },
    )?;
    state::save(&app_state)?;
    Ok(name)
}

#[tauri::command(rename_all = "snake_case")]
async fn create_profile_from_collection(
    name: String,
    description: Option<String>,
    collection_id: u64,
    selected_mod_ids: Vec<String>,
    include_pending: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        create_profile_from_collection_impl(
            name,
            description,
            collection_id,
            selected_mod_ids,
            include_pending,
        )
    })
    .await
    .map_err(|err| format!("Collection profile task failed: {err}"))?
    .map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
async fn profile_collection_preview(name: String) -> Result<workshop::CollectionPreview, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<workshop::CollectionPreview> {
        let app_state = state::load()?;
        let profile = app_state
            .profiles
            .iter()
            .find(|profile| profile.name == name)
            .with_context(|| format!("Profile not found: {name}"))?;
        let source = profile
            .source
            .as_ref()
            .with_context(|| format!("Profile has no Steam collection: {name}"))?;
        workshop::preview_collection(source.id)
    })
    .await
    .map_err(|err| format!("Collection task failed: {err}"))?
    .map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
async fn add_collection_mods_to_profile(
    name: String,
    selected_mod_ids: Vec<String>,
) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<usize> {
        let mut app_state = state::load()?;
        let profile_index = app_state
            .profiles
            .iter()
            .position(|profile| profile.name == name)
            .with_context(|| format!("Profile not found: {name}"))?;
        let source_id = app_state.profiles[profile_index]
            .source
            .as_ref()
            .map(|source| source.id)
            .with_context(|| format!("Profile has no Steam collection: {name}"))?;
        let preview = workshop::preview_collection(source_id)?;
        let installed: HashMap<&str, &workshop::CollectionItem> = preview
            .items
            .iter()
            .filter(|item| item.installed)
            .filter_map(|item| item.mod_id.as_deref().map(|id| (id, item)))
            .collect();
        let added;
        {
            let profile = &mut app_state.profiles[profile_index];
            let before = profile.active_ids.len();
            for id in &selected_mod_ids {
                if installed.contains_key(id.as_str()) && !profile.active_ids.contains(id) {
                    profile.active_ids.push(id.clone());
                }
            }
            let active_set: HashSet<&str> = profile.active_ids.iter().map(String::as_str).collect();
            profile.pending_workshop_ids = preview
                .items
                .iter()
                .filter(|item| item.status == "missing")
                .map(|item| item.workshop_id)
                .collect();
            profile.collection_children = preview
                .items
                .iter()
                .filter(|item| item.status != "other_game")
                .map(|item| item.workshop_id)
                .collect();
            profile.excluded_workshop_ids = preview
                .items
                .iter()
                .filter(|item| item.status == "installed")
                .filter(|item| {
                    item.mod_id
                        .as_deref()
                        .is_some_and(|id| !active_set.contains(id))
                })
                .map(|item| item.workshop_id)
                .collect();
            if let Some(source) = profile.source.as_mut() {
                source.title = preview.title.clone();
            }
            profile.updated = now_unix();
            added = profile.active_ids.len().saturating_sub(before);
        }
        state::save(&app_state)?;
        Ok(added)
    })
    .await
    .map_err(|err| format!("Collection task failed: {err}"))?
    .map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn preview_auto_categories(overwrite: bool) -> Result<auto_category::AutoPreview, String> {
    auto_category::preview(overwrite).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn apply_auto_categories(overwrite: bool) -> Result<auto_category::AutoApplyResult, String> {
    if aml_running() {
        return Err("Close AML before changing categories.".to_string());
    }
    auto_category::apply(overwrite).map_err(|err| format!("{err:#}"))
}

#[tauri::command]
fn clear_category_feedback() -> Result<usize, String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    let count = state::clear_category_feedback(&mut app_state);
    state::save(&app_state).map_err(|err| format!("{err:#}"))?;
    Ok(count)
}

#[tauri::command(rename_all = "snake_case")]
fn clear_mod_category_feedback(mod_id: String) -> Result<bool, String> {
    let mut app_state = state::load().map_err(|err| format!("{err:#}"))?;
    let removed = state::clear_mod_feedback(&mut app_state, &mod_id);
    state::save(&app_state).map_err(|err| format!("{err:#}"))?;
    Ok(removed)
}

#[tauri::command(rename_all = "snake_case")]
fn list_mod_config_files(mod_id: String) -> Result<Vec<config_editor::ConfigFileEntry>, String> {
    config_editor::list_files(&mod_id).map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
fn read_mod_config(mod_id: String, path: String) -> Result<config_editor::ConfigFile, String> {
    config_editor::read(&mod_id, &path).map_err(|err| format!("{err:#}"))
}

#[tauri::command(rename_all = "snake_case")]
fn save_mod_config(
    mod_id: String,
    path: String,
    content: String,
) -> Result<config_editor::ConfigSaveResult, String> {
    if aml_running() {
        return Err("Close AML before changing config.".to_string());
    }
    if game_running() {
        return Err("Close XCOM 2 before changing config.".to_string());
    }
    config_editor::save(&mod_id, &path, &content).map_err(|err| format!("{err:#}"))
}

fn main() {
    // Small headless hooks, useful for debugging and for testing the backend
    // without opening a window. They never launch the game.
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--headless-auto-categories") {
        match auto_category::preview(false) {
            Ok(preview) => {
                println!(
                    "total: {}, unsorted: {}, already_categorized: {}, assignable: {}",
                    preview.total,
                    preview.unsorted,
                    preview.already_categorized,
                    preview.assignable
                );
                for group in preview.groups {
                    println!("{:4}  {}", group.count, group.name);
                    for entry in group.mods.iter().take(3) {
                        println!("        - {}", entry.name);
                    }
                }
            }
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args
        .iter()
        .any(|arg| arg == "--headless-auto-categories-apply")
    {
        match auto_category::apply(false) {
            Ok(result) => println!("{result:#?}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(index) = args.iter().position(|arg| arg == "--headless-readme") {
        let Some(mod_id) = args.get(index + 1) else {
            eprintln!("Usage: --headless-readme <mod_id>");
            std::process::exit(2);
        };
        match readme::read(mod_id) {
            Ok(content) => println!("{content:#?}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(index) = args.iter().position(|arg| arg == "--headless-config-list") {
        let Some(mod_id) = args.get(index + 1) else {
            eprintln!("Usage: --headless-config-list <mod_id>");
            std::process::exit(2);
        };
        match config_editor::list_files(mod_id) {
            Ok(files) => println!("{files:#?}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(index) = args.iter().position(|arg| arg == "--headless-config-read") {
        let (Some(mod_id), Some(path)) = (args.get(index + 1), args.get(index + 2)) else {
            eprintln!("Usage: --headless-config-read <mod_id> <Config/File.ini>");
            std::process::exit(2);
        };
        match config_editor::read(mod_id, path) {
            Ok(file) => println!("{file:#?}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(index) = args
        .iter()
        .position(|arg| arg == "--headless-import-collection")
    {
        let Some(id) = args.get(index + 1).and_then(|id| id.parse::<u64>().ok()) else {
            eprintln!("Usage: --headless-import-collection <published_file_id> [name]");
            std::process::exit(2);
        };
        let name = args.get(index + 2).cloned().unwrap_or_default();
        match workshop::preview_collection(id) {
            Ok(preview) => {
                let selected: Vec<String> = preview
                    .items
                    .iter()
                    .filter(|item| item.installed)
                    .filter_map(|item| item.mod_id.clone())
                    .collect();
                match create_profile_from_collection_impl(name, None, id, selected, true) {
                    Ok(name) => println!("{name}"),
                    Err(err) => {
                        eprintln!("{err:#}");
                        std::process::exit(1);
                    }
                }
            }
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(index) = args
        .iter()
        .position(|arg| arg == "--headless-delete-profile")
    {
        let Some(name) = args.get(index + 1) else {
            eprintln!("Usage: --headless-delete-profile <name>");
            std::process::exit(2);
        };
        match state::load().and_then(|mut app_state| {
            state::delete_profile(&mut app_state, name)?;
            state::save(&app_state)
        }) {
            Ok(()) => println!("deleted {name}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(index) = args.iter().position(|arg| arg == "--headless-collection") {
        let Some(id) = args.get(index + 1).and_then(|id| id.parse::<u64>().ok()) else {
            eprintln!("Usage: --headless-collection <published_file_id>");
            std::process::exit(2);
        };
        match workshop::preview_collection(id) {
            Ok(preview) => {
                println!(
                    "{} — total: {}, installed: {}, missing: {}, other_game: {}",
                    preview.title,
                    preview.total,
                    preview.installed,
                    preview.missing,
                    preview.other_game
                );
                for item in preview.items.iter().take(20) {
                    println!(
                        "  {}\t{}\t{}",
                        item.workshop_id,
                        item.status,
                        item.title.as_deref().unwrap_or("?")
                    );
                }
            }
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|arg| arg == "--headless-mods") {
        match load_snapshot() {
            Ok(snapshot) => {
                for entry in snapshot.mods {
                    println!(
                        "{}\t{}\t{}\t{}",
                        entry.id,
                        entry.name,
                        entry
                            .workshop_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        entry.category
                    );
                }
            }
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|arg| arg == "--headless-state") {
        match load_snapshot() {
            Ok(snapshot) => {
                println!("{:#?}", snapshot.summary);
                println!("{:#?}", snapshot.paths);
                println!("active_ids: {}", snapshot.active_ids.len());
            }
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|arg| arg == "--headless-save") {
        let result = load_snapshot().and_then(|snapshot| save_state_impl(snapshot.active_ids));
        match result {
            Ok(saved) => println!("{saved:#?}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|arg| arg == "--headless-workshop") {
        match workshop::refresh() {
            Ok(result) => println!("{result:#?}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    if args.iter().any(|arg| arg == "--headless-diagnostics") {
        match diagnostics::scan() {
            Ok(result) => println!("{result:#?}"),
            Err(err) => {
                eprintln!("{err:#}");
                std::process::exit(1);
            }
        }
        return;
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_state,
            save_state,
            launch_game,
            refresh_workshop,
            scan_diagnostics,
            get_mod_details,
            get_mod_readme,
            open_workshop,
            set_mod_category,
            create_category,
            rename_category,
            delete_category,
            save_profile,
            delete_profile,
            get_profile,
            preview_steam_collection,
            create_empty_profile,
            create_profile_from_collection,
            profile_collection_preview,
            add_collection_mods_to_profile,
            preview_auto_categories,
            apply_auto_categories,
            clear_category_feedback,
            clear_mod_category_feedback,
            list_mod_config_files,
            read_mod_config,
            save_mod_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Tauri application");
}
