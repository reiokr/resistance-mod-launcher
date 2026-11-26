//! Read-only AML import prototype.
//!
//! It imports AML's settings.json, resolves the active Workshop mods and
//! generates a candidate XComModOptions.ini in /tmp. It never writes to the
//! game config.

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use xcom2_launcher_core::{
    default_mod_options_timestamp, diff_order, discover_paths, parse_active_mods,
    parse_ini_version, render_mod_options, resolve_active_mods, AmlRoot,
};

#[derive(Debug, Parser)]
#[command(
    name = "aml-preview",
    version,
    about = "Import AML settings and preview XComModOptions.ini (read-only)"
)]
struct Args {
    /// AML settings.json (default: auto-discovered from the game's Launcher dir)
    #[arg(long)]
    settings: Option<PathBuf>,

    /// Generate the vanilla XCOM 2 config instead of War of the Chosen
    #[arg(long)]
    vanilla: bool,

    /// Output path (default: /tmp/xcom2-launcher-linux/XComModOptions.ini)
    #[arg(long)]
    out: Option<PathBuf>,

    /// Print the generated INI to stdout instead of writing a file
    #[arg(long)]
    stdout: bool,

    /// Do not write the generated INI anywhere
    #[arg(long)]
    no_write: bool,

    /// Machine-readable JSON summary
    #[arg(long)]
    json: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let paths = discover_paths()?;

    let settings_path = args
        .settings
        .clone()
        .or_else(|| paths.settings.clone())
        .context("AML settings.json was not found; pass --settings <path>")?;
    let settings = AmlRoot::load(&settings_path)?;

    let current_config = if args.vanilla {
        paths.vanilla_config.clone()
    } else {
        paths.wotc_config.clone()
    }
    .context("Game config directory was not found (is the Proton prefix present?)")?;

    let current_path = current_config.join("XComModOptions.ini");
    let current_text = fs::read_to_string(&current_path)
        .with_context(|| format!("Could not read {}", current_path.display()))?;
    let current_ids = parse_active_mods(&current_text);
    let current_version = parse_ini_version(&current_text);

    let active = resolve_active_mods(&settings, &paths)?;
    let missing: Vec<_> = active.iter().filter(|module| module.missing).collect();

    let timestamp = paths
        .game_dir
        .as_deref()
        .and_then(|game_dir| default_mod_options_timestamp(game_dir, !args.vanilla).ok())
        .or_else(|| current_version.clone())
        .unwrap_or_else(|| "0.000000".to_string());

    let generated = render_mod_options(&active, &timestamp);
    let generated_ids: Vec<String> = active
        .iter()
        .map(|module| module.internal_id.clone())
        .collect();
    let diff = diff_order(&current_ids, &generated_ids);

    let out_path = args
        .out
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join("xcom2-launcher-linux/XComModOptions.ini"));

    if args.stdout {
        print!("{generated}");
    } else if !args.no_write {
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Could not create {}", parent.display()))?;
        }
        fs::write(&out_path, generated.as_bytes())
            .with_context(|| format!("Could not write {}", out_path.display()))?;
    }

    let all_mods = settings.all_mods().len();

    if args.json {
        let summary = serde_json::json!({
            "game_dir": paths.game_dir,
            "prefix": paths.prefix,
            "workshop_dir": paths.workshop_dir,
            "settings": settings_path,
            "mods": all_mods,
            "active": active.len(),
            "missing": missing.len(),
            "missing_ids": missing.iter().map(|m| m.internal_id.clone()).collect::<Vec<_>>(),
            "current_config": current_path,
            "current_active": current_ids.len(),
            "generated_active": generated_ids.len(),
            "ini_version_generated": timestamp,
            "ini_version_current": current_version,
            "order_identical": diff.identical,
            "first_mismatch": diff.first_mismatch,
            "missing_from_generated": diff.missing_from_generated,
            "extra_in_generated": diff.extra_in_generated,
            "output": if args.no_write || args.stdout { None } else { Some(out_path) },
        });
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    println!("XCOM 2 native core prototype (read-only)");
    println!("---------------------------------------");
    println!("Game:            {}", display_opt(&paths.game_dir));
    println!("Proton prefix:   {}", display_opt(&paths.prefix));
    println!("Workshop:        {}", display_opt(&paths.workshop_dir));
    println!("AML settings:    {} (mods: {all_mods})", settings_path.display());
    println!("Current config:  {} (ActiveMods: {})", current_path.display(), current_ids.len());
    println!(
        "Active resolved: {} (missing: {})",
        active.len(),
        missing.len()
    );
    if !missing.is_empty() {
        let names: Vec<_> = missing
            .iter()
            .take(10)
            .map(|module| module.internal_id.as_str())
            .collect();
        println!("  missing:       {}", names.join(", "));
    }
    println!("IniVersion:      generated {timestamp}, current {}", current_version.as_deref().unwrap_or("<none>"));
    println!("Order identical: {}", if diff.identical { "yes" } else { "no" });
    if let Some(index) = diff.first_mismatch {
        println!("  first mismatch at position {index}");
    }
    if !diff.missing_from_generated.is_empty() {
        println!(
            "  in current but not generated: {}",
            diff.missing_from_generated.join(", ")
        );
    }
    if !diff.extra_in_generated.is_empty() {
        println!(
            "  generated but not in current: {}",
            diff.extra_in_generated.join(", ")
        );
    }

    if args.no_write {
        println!("Generated INI:   not written (--no-write)");
    } else if args.stdout {
        // already printed
    } else {
        println!("Generated INI:   {}", out_path.display());
        println!("Compare with:    diff -u <(grep '^ActiveMods=' '{}') <(grep '^ActiveMods=' '{}')", current_path.display(), out_path.display());
    }

    Ok(())
}

fn display_opt(value: &Option<PathBuf>) -> String {
    value
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "not found".to_string())
}
