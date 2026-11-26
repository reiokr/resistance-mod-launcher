//! Rule-based automatic categories from Workshop tags, mod names and
//! descriptions. Only native categories are touched; `XComModOptions.ini` is
//! never modified.

use crate::workshop::{self, WorkshopCache};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use xcom2_launcher_core::{AmlMod, AmlRoot, discover_paths};

const CATEGORY_ORDER: &[&str] = &[
    "Amalgamation",
    "Long War",
    "Fixes & Tools",
    "Localization",
    "Audio",
    "Classes",
    "Factions",
    "Cosmetics",
    "UI",
    "Maps & Missions",
    "Enemies & Aliens",
    "Strategy & Facilities",
    "Weapons & Items",
    "Gameplay & Balance",
];

const UNSORTED: &str = "Unsorted";

struct Rule {
    category: &'static str,
    tags: &'static [&'static str],
    names: &'static [&'static str],
    descriptions: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule {
        category: "Amalgamation",
        tags: &["amalgamation", "amalgamation specs"],
        names: &["amalgamation", "amalgam"],
        descriptions: &[],
    },
    Rule {
        category: "Long War",
        tags: &[
            "long war",
            "long war of the chosen",
            "lwotc",
            "lw2",
            "long war 2",
        ],
        names: &["long war", "lwotc", "lw2"],
        descriptions: &[],
    },
    Rule {
        category: "Fixes & Tools",
        tags: &[
            "bugfix",
            "bugfixes",
            "fix",
            "fixes",
            "modfix",
            "qol",
            "utility",
            "template master",
            "mod tools",
            "modding",
        ],
        names: &[
            "fix",
            "fixes",
            "fixed",
            "patch",
            "bug",
            "qol",
            "utility",
            "highlander",
            "template master",
            "mod tools",
            "missing mods",
            "cleaner",
            "update",
            "bandaid",
            "missing out",
        ],
        descriptions: &["bug fix", "bugfix", "quality of life", "missing packages"],
    },
    Rule {
        category: "Localization",
        tags: &["translation", "localization", "localisation", "language"],
        names: &[
            "translation",
            "localization",
            "localisation",
            "language",
            "english",
        ],
        descriptions: &[],
    },
    Rule {
        category: "Audio",
        tags: &["sound", "sounds", "audio", "music", "voice", "voices"],
        names: &[
            "sound",
            "sounds",
            "audio",
            "music",
            "voice",
            "voices",
            "voicepack",
            "voicepacks",
            "shut up",
            "quieter",
            "quiet",
            "tygan",
        ],
        descriptions: &["soundtrack", "voice pack", "music"],
    },
    Rule {
        category: "Classes",
        tags: &["soldier class", "class", "specialization"],
        names: &[
            "class",
            "classes",
            "perk",
            "perk pack",
            "specialization",
            "specialisation",
            "spec",
            "awc",
            "skills",
            "skill",
            "mystic",
            "psionics",
        ],
        descriptions: &["new soldier class", "new class"],
    },
    Rule {
        category: "Factions",
        tags: &["faction", "factions", "resistance"],
        names: &[
            "faction",
            "factions",
            "resistance",
            "raider",
            "insurgents",
            "insurgent",
        ],
        descriptions: &["new faction"],
    },
    Rule {
        category: "Cosmetics",
        tags: &[
            "soldier customization",
            "cosmetic",
            "cosmetics",
            "fashion before comfort",
            "hair",
            "face",
            "color",
            "colour",
            "pattern",
            "texture",
            "skin",
        ],
        names: &[
            "hair",
            "face",
            "faces",
            "uniform",
            "camo",
            "camouflage",
            "color",
            "colour",
            "colours",
            "colors",
            "pattern",
            "patterns",
            "texture",
            "cosmetic",
            "customization",
            "customisation",
            "customize",
            "custom",
            "tattoo",
            "portrait",
            "fashion",
            "skin",
            "arms",
            "shoulders",
            "torso",
            "legs",
            "cape",
            "cloak",
            "gear",
            "wedding",
            "nations",
            "nationality",
            "flag",
            "scar",
            "scars",
            "eyepatch",
            "hood",
            "deco",
            "fx",
            "accessories",
            "accessory",
            "appearance",
            "headwear",
            "heads",
            "limbs",
            "pose",
            "poses",
            "disguises",
            "invisible",
            "skull",
            "animation",
            "animations",
            "cosmetics",
        ],
        descriptions: &["cosmetic", "customization", "customisation", "more nations"],
    },
    Rule {
        category: "UI",
        tags: &["ui", "icons", "icon", "hud"],
        names: &[
            "ui",
            "hud",
            "interface",
            "tooltip",
            "icon",
            "icons",
            "screen",
            "menu",
            "preview",
            "zoom",
            "camera",
            "debriefing",
            "information",
            "info",
            "evac",
            "squad select",
            "bonds",
            "mission names",
            "visualize",
            "waste my time",
            "encounter list",
            "loadout manager",
            "manual loadout",
            "loot indicator",
            "controller",
            "controllers",
            "gotcha",
            "show upgrades",
            "drop down",
            "scans",
            "unit scans",
            "sensible wounds",
        ],
        descriptions: &["user interface", "hud", "tooltip"],
    },
    Rule {
        category: "Maps & Missions",
        tags: &[
            "map", "maps", "mission", "missions", "plot", "plots", "sitrep", "sitreps",
        ],
        names: &[
            "map",
            "maps",
            "mission",
            "missions",
            "plot",
            "plots",
            "sitrep",
            "sitreps",
            "objective",
            "clouds",
            "skies",
            "poi",
            "pois",
            "situation report",
        ],
        descriptions: &["new map", "map pack", "new plot", "parcels"],
    },
    Rule {
        category: "Enemies & Aliens",
        tags: &[
            "alien", "aliens", "enemy", "enemies", "advent", "muton", "mutons", "chosen", "lost",
            "drone", "drones",
        ],
        names: &[
            "alien",
            "aliens",
            "enemy",
            "enemies",
            "advent",
            "muton",
            "mutons",
            "viper",
            "vipers",
            "sectoid",
            "sectoids",
            "chosen",
            "lost",
            "drone",
            "drones",
            "sectopod",
            "sectopods",
            "berserker",
            "berserkers",
            "turret",
            "turrets",
            "chryssalid",
            "chryssalids",
            "aba",
            "better advent",
            "archon",
            "archons",
            "ruler",
            "rulers",
            "zombie",
            "zombies",
        ],
        descriptions: &["new enemy", "new alien", "new advent"],
    },
    Rule {
        category: "Strategy & Facilities",
        tags: &["facility", "facilities", "strategy", "geoscape"],
        names: &[
            "facility",
            "facilities",
            "avenger",
            "base",
            "research",
            "engineer",
            "strategy",
            "geoscape",
            "cities",
            "city",
            "power cores",
            "power core",
            "breakthrough",
            "breakthroughs",
            "scientist",
            "events",
            "continent",
            "bonus",
            "class requirements",
            "avatar project",
            "covert slot",
            "living space",
            "armoury",
            "armory",
            "reward deck",
            "reward decks",
            "gts",
        ],
        descriptions: &[
            "new breakthrough",
            "breakthrough",
            "facility",
            "avenger",
            "strategy layer",
            "research",
        ],
    },
    Rule {
        category: "Weapons & Items",
        tags: &[
            "weapon",
            "weapons",
            "item",
            "items",
            "armor",
            "armour",
            "primary",
            "secondary",
            "equipment",
        ],
        names: &[
            "weapon",
            "weapons",
            "gun",
            "guns",
            "rifle",
            "rifles",
            "pistol",
            "pistols",
            "sword",
            "swords",
            "axe",
            "axes",
            "armor",
            "armour",
            "armors",
            "item",
            "items",
            "equipment",
            "launcher",
            "launchers",
            "cannon",
            "cannons",
            "shotgun",
            "shotguns",
            "sniper",
            "snipers",
            "blade",
            "blades",
            "shield",
            "shields",
            "ammo",
            "grenade",
            "grenades",
            "bomb",
            "bombs",
            "rounds",
            "plating",
            "coilgun",
            "coilguns",
            "vest",
            "vests",
            "autopistol",
            "autopistols",
            "loot",
            "munitions",
            "shells",
            "gauntlets",
            "smg",
            "smgs",
            "caliber",
            "vektor",
            "beam",
            "laser",
            "tesla",
            "exo",
            "upgrades",
            "smite",
        ],
        descriptions: &["new weapon", "new armor", "new item"],
    },
    Rule {
        category: "Gameplay & Balance",
        tags: &[
            "gameplay",
            "ability",
            "abilities",
            "perk",
            "perks",
            "soldier",
            "balance",
            "mechanic",
            "mechanics",
            "traits",
            "trait",
            "tactical",
            "reload",
        ],
        names: &[
            "gameplay",
            "balance",
            "ability",
            "abilities",
            "perk",
            "perks",
            "overhaul",
            "mechanic",
            "mechanics",
            "soldier",
            "squad",
            "squads",
            "training",
            "combat",
            "traits",
            "trait",
            "tactical",
            "reload",
            "tweak",
            "tweaks",
            "ai",
            "stun",
            "second wave",
            "bondmate",
            "bondmates",
            "status",
            "penalty",
            "penalties",
            "explosive",
            "explosives",
            "demolition",
            "overwatch",
            "defense",
            "defence",
            "hero",
            "heroes",
            "actions",
            "heal",
            "healing",
            "infiltration",
            "parry",
            "fight",
            "fights",
            "cooldown",
            "teamwork",
            "spark",
            "sparks",
            "alert",
            "setback",
            "knowledge",
            "buff",
            "potential",
            "mind control",
            "bleedout",
            "anarchy",
            "sight range",
            "force level",
            "variable",
            "corpses",
            "corpse",
            "hacking",
            "loadout",
            "loadouts",
            "gatecrasher",
            "rescue",
            "revival",
            "revive",
            "suppression",
            "bladestorm",
            "soldiers",
            "rookies",
            "rookie",
            "pool soldier",
            "pool soldiers",
            "carried units",
            "relay",
            "visibility",
            "rapid fire",
            "not created equal",
            "hidden",
            "extract",
            "narrative",
            "playable",
            "steal",
            "priest",
            "stasis",
        ],
        descriptions: &[
            "adds an ability",
            "new ability",
            "stabilization",
            "aim penalty",
            "gameplay",
            "balance",
            "overhaul",
            "overwatch",
        ],
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct AutoMod {
    pub id: String,
    pub name: String,
    pub current: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoGroup {
    pub name: String,
    pub count: usize,
    pub mods: Vec<AutoMod>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoPreview {
    pub total: usize,
    pub unsorted: usize,
    pub already_categorized: usize,
    pub assignable: usize,
    pub manual_pinned: usize,
    pub learned: usize,
    pub rule_based: usize,
    pub training_examples: usize,
    pub training_categories: usize,
    pub groups: Vec<AutoGroup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoApplyResult {
    pub assigned: usize,
    pub categories_created: usize,
    pub manual_pinned: usize,
    pub learned: usize,
    pub rule_based: usize,
    pub backup: Option<String>,
}

struct Assignment {
    id: String,
    name: String,
    current: String,
    category: String,
    source: &'static str,
}

struct Plan {
    total: usize,
    unsorted: usize,
    already_categorized: usize,
    manual_pinned: usize,
    learned: usize,
    rule_based: usize,
    training_examples: usize,
    training_categories: usize,
    assignments: Vec<Assignment>,
}

pub(crate) fn normalize_tag(tag: &str) -> String {
    tag.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(crate) fn normalize_name(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch.is_alphanumeric() {
            if ch.is_uppercase() && index > 0 {
                let prev = chars[index - 1];
                let next = chars.get(index + 1).copied();
                let next_is_lower = next.is_some_and(|next| next.is_lowercase());
                let plural_s = next == Some('s');
                if prev.is_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_uppercase() && next_is_lower && !plural_s)
                {
                    out.push(' ');
                }
            }
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn has_phrase(padded_name: &str, phrase: &str) -> bool {
    padded_name.contains(&format!(" {phrase} "))
}

fn classify(name: &str, tags: &[String], description: &str) -> &'static str {
    let normalized = normalize_name(name);
    if normalized == "fix"
        || normalized.ends_with(" fix")
        || normalized.ends_with(" fixes")
        || normalized.ends_with(" fixed")
        || normalized.starts_with("fix ")
        || normalized.starts_with("fixes ")
    {
        return "Fixes & Tools";
    }
    let match_name = normalized
        .replace("war of the chosen", " ")
        .replace("wotc", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let padded_name = format!(" {match_name} ");
    let single_token = !match_name.is_empty() && !match_name.contains(' ');
    let description = description.to_lowercase();
    let short_description: String = description.chars().take(1600).collect();

    let mut best_category = UNSORTED;
    let mut best_score = 0;
    for rule in RULES {
        let mut score = 0;
        for tag in tags {
            if rule.tags.iter().any(|keyword| *keyword == tag) {
                score += 6;
            }
        }
        for phrase in rule.names {
            if has_phrase(&padded_name, phrase) {
                score += 8;
            } else if single_token && phrase.len() >= 5 && match_name.contains(phrase) {
                score += 8;
            }
        }
        if score > best_score {
            best_category = rule.category;
            best_score = score;
        }
    }
    if best_score > 0 {
        return best_category;
    }

    for rule in RULES {
        let score = rule
            .descriptions
            .iter()
            .filter(|pattern| short_description.contains(*pattern))
            .count();
        if score > best_score {
            best_category = rule.category;
            best_score = score;
        }
    }
    best_category
}

fn current_category(entry: &AmlMod, state: &crate::state::PersistedState) -> String {
    state
        .mod_categories
        .get(&entry.id)
        .cloned()
        .or_else(|| entry.category.clone())
        .unwrap_or_else(|| UNSORTED.to_string())
}

fn build_plan(
    state: &crate::state::PersistedState,
    cache: &WorkshopCache,
    overwrite: bool,
) -> Result<Plan> {
    let paths = discover_paths()?;
    let settings_path = paths
        .settings
        .clone()
        .context("AML settings.json was not found")?;
    let settings = AmlRoot::load(&settings_path)?;
    let all = settings.all_mods_with_workshop(paths.workshop_dir.as_deref())?;
    let model = crate::category_learning::train(&state.category_feedback, &all, cache);

    let mut assignments = Vec::new();
    let mut unsorted = 0usize;
    let mut already_categorized = 0usize;
    let mut manual_pinned = 0usize;
    let mut learned = 0usize;
    let mut rule_based = 0usize;
    for entry in &all {
        let current = current_category(entry, state);
        let manual = state.category_feedback.contains_key(&entry.id);
        if manual {
            manual_pinned += 1;
            if current == UNSORTED {
                unsorted += 1;
            } else {
                already_categorized += 1;
            }
            continue;
        }
        if current == UNSORTED {
            unsorted += 1;
        } else {
            already_categorized += 1;
            if !overwrite {
                continue;
            }
        }

        let workshop_item = entry
            .workshop_id
            .and_then(|id| cache.items.get(&id.to_string()));
        let tags: Vec<String> = workshop_item
            .map(|details| details.tags.iter().map(|tag| normalize_tag(tag)).collect())
            .unwrap_or_default();
        let description = workshop_item
            .and_then(|details| details.description.clone())
            .or_else(|| entry.description.clone())
            .unwrap_or_default();
        let name = entry.name.clone().unwrap_or_else(|| entry.id.clone());

        let prediction = model.as_ref().and_then(|model| model.predict(entry, cache));
        let (category, source) = match prediction {
            Some(prediction) if prediction.category != UNSORTED => (prediction.category, "learned"),
            _ => (classify(&name, &tags, &description).to_string(), "rules"),
        };
        if category == UNSORTED {
            continue;
        }
        if source == "learned" {
            learned += 1;
        } else {
            rule_based += 1;
        }
        assignments.push(Assignment {
            id: entry.id.clone(),
            name,
            current,
            category,
            source,
        });
    }

    assignments.sort_by(|a, b| {
        let index = |category: &str| {
            CATEGORY_ORDER
                .iter()
                .position(|name| *name == category)
                .unwrap_or(CATEGORY_ORDER.len())
        };
        index(&a.category)
            .cmp(&index(&b.category))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });

    Ok(Plan {
        total: all.len(),
        unsorted,
        already_categorized,
        manual_pinned,
        learned,
        rule_based,
        training_examples: model
            .as_ref()
            .map(|model| model.training_examples)
            .unwrap_or(0),
        training_categories: model
            .as_ref()
            .map(|model| model.training_categories)
            .unwrap_or(0),
        assignments,
    })
}

fn group_assignments(assignments: &[Assignment]) -> Vec<AutoGroup> {
    let mut groups: HashMap<String, Vec<AutoMod>> = HashMap::new();
    for assignment in assignments {
        groups
            .entry(assignment.category.clone())
            .or_default()
            .push(AutoMod {
                id: assignment.id.clone(),
                name: assignment.name.clone(),
                current: assignment.current.clone(),
                source: assignment.source.to_string(),
            });
    }
    let mut result: Vec<AutoGroup> = groups
        .into_iter()
        .map(|(name, mods)| AutoGroup {
            name,
            count: mods.len(),
            mods,
        })
        .collect();
    result.sort_by(|a, b| {
        let index = |category: &str| {
            CATEGORY_ORDER
                .iter()
                .position(|name| *name == category)
                .unwrap_or(CATEGORY_ORDER.len())
        };
        index(&a.name).cmp(&index(&b.name))
    });
    result
}

pub fn preview(overwrite: bool) -> Result<AutoPreview> {
    let state = crate::state::load()?;
    let cache = workshop::load_cache()?;
    let plan = build_plan(&state, &cache, overwrite)?;
    let groups = group_assignments(&plan.assignments);
    Ok(AutoPreview {
        total: plan.total,
        unsorted: plan.unsorted,
        already_categorized: plan.already_categorized,
        assignable: plan.assignments.len(),
        manual_pinned: plan.manual_pinned,
        learned: plan.learned,
        rule_based: plan.rule_based,
        training_examples: plan.training_examples,
        training_categories: plan.training_categories,
        groups,
    })
}

fn backup_state() -> Result<Option<String>> {
    let path = crate::app_data_dir()?.join("state.json");
    let Ok(bytes) = fs::read(&path) else {
        return Ok(None);
    };
    let dir = crate::app_data_dir()?.join("backups");
    fs::create_dir_all(&dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    for attempt in 0..1000 {
        let name = if attempt == 0 {
            format!("state-{stamp}.json")
        } else {
            format!("state-{stamp}-{attempt}.json")
        };
        let target = dir.join(name);
        if !target.exists() {
            fs::write(&target, &bytes)?;
            return Ok(Some(target.display().to_string()));
        }
    }
    Ok(None)
}

pub fn apply(overwrite: bool) -> Result<AutoApplyResult> {
    let cache = workshop::load_cache()?;
    let mut state = crate::state::load()?;
    let plan = build_plan(&state, &cache, overwrite)?;
    if plan.assignments.is_empty() {
        return Ok(AutoApplyResult {
            assigned: 0,
            categories_created: 0,
            manual_pinned: plan.manual_pinned,
            learned: 0,
            rule_based: 0,
            backup: None,
        });
    }

    let backup = backup_state()?;
    let before = state.categories.len();
    let mut assigned_ids = HashSet::new();
    for assignment in &plan.assignments {
        if !assigned_ids.insert(assignment.id.clone()) {
            continue;
        }
        crate::state::set_mod_category(&mut state, &assignment.id, &assignment.category)?;
    }
    let categories_created = state.categories.len().saturating_sub(before);
    crate::state::save(&state)?;

    Ok(AutoApplyResult {
        assigned: assigned_ids.len(),
        categories_created,
        manual_pinned: plan.manual_pinned,
        learned: plan.learned,
        rule_based: plan.rule_based,
        backup,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camel_case_names_are_split() {
        assert_eq!(
            normalize_name("FreeReloadAnytimeWOTC"),
            "free reload anytime wotc"
        );
        assert_eq!(
            normalize_name("ArmsShouldersCrossTier"),
            "arms shoulders cross tier"
        );
        assert_eq!(
            normalize_name("robojumperSquadSelect"),
            "robojumper squad select"
        );
        assert_eq!(normalize_name("Restored SITREPs"), "restored sitreps");
        assert_eq!(normalize_name("SPARKs"), "sparks");
    }

    #[test]
    fn tag_and_name_rules() {
        assert_eq!(
            classify("Beam Grenade Launcher", &["weapon".into()], ""),
            "Weapons & Items"
        );
        assert_eq!(
            classify("Maps by Vozati", &["map".into()], ""),
            "Maps & Missions"
        );
        assert_eq!(
            classify("Amalgamation: Knight Specialization", &[], ""),
            "Amalgamation"
        );
        assert_eq!(
            classify("Avenger Defense Bondmates Fix", &[], ""),
            "Fixes & Tools"
        );
        assert_eq!(classify("robojumperSquadSelect", &[], ""), "UI");
        assert_eq!(classify("ExpandedWotCMissionNames", &[], ""), "UI");
        assert_eq!(
            classify(
                "One Million Spark Colours",
                &["ui".into(), "modfix".into()],
                ""
            ),
            "Cosmetics"
        );
        assert_eq!(
            classify("Coilguns for War of the Chosen", &[], ""),
            "Weapons & Items"
        );
        assert_eq!(
            classify("Restored SITREPs", &["gameplay".into()], ""),
            "Maps & Missions"
        );
        assert_eq!(classify("WOTC_GremlinLoot", &[], ""), "Weapons & Items");
        assert_eq!(classify("strongerrookies", &[], ""), "Gameplay & Balance");
        assert_eq!(
            classify("expandedcontinentbonuses", &[], ""),
            "Strategy & Facilities"
        );
        assert_eq!(classify("Xlegendsheads", &[], ""), "Cosmetics");
        assert_eq!(
            classify("WOTCIridarAppearanceManager", &[], ""),
            "Cosmetics"
        );
        assert_eq!(
            classify("WOTC_FasterBeamVektor", &[], ""),
            "Weapons & Items"
        );
        assert_eq!(
            classify("No Drop Down Lists For Controllers", &[], ""),
            "UI"
        );
    }

    #[test]
    fn description_fallback_is_last_resort() {
        assert_eq!(
            classify("Some Mod", &[], "Adds 14 new breakthroughs to the game."),
            "Strategy & Facilities"
        );
        assert_eq!(
            classify("Some Mod", &[], "Adds an ability that stuns enemies."),
            "Gameplay & Balance"
        );
        assert_eq!(
            classify("Some Mod", &[], "Nothing useful here."),
            "Unsorted"
        );
    }
}
