//! Small Naive Bayes learner over manual category corrections.
//!
//! Training data is only what the user changed by hand (state.category_feedback),
//! never the automatic assignments. Features are Workshop tags (weight 3), mod
//! name tokens (weight 2) and description tokens (weight 1). A prediction is
//! used only when it is reasonably confident; otherwise the rule classifier
//! remains in charge.

use crate::auto_category::{normalize_name, normalize_tag};
use crate::state::CategoryFeedback;
use crate::workshop::WorkshopCache;
use std::collections::{HashMap, HashSet};
use xcom2_launcher_core::{AmlMod, AmlRoot};

const MIN_EXAMPLES: usize = 5;
const MIN_CATEGORIES: usize = 2;
const MIN_PROBABILITY: f64 = 0.55;
const MIN_MARGIN: f64 = 0.15;
const MAX_DESCRIPTION_TOKENS: usize = 40;

#[derive(Debug, Clone)]
pub struct Prediction {
    pub category: String,
}

#[derive(Debug, Clone, Default)]
pub struct Model {
    total_docs: usize,
    doc_counts: HashMap<String, usize>,
    feature_totals: HashMap<String, usize>,
    feature_counts: HashMap<String, HashMap<String, usize>>,
    vocab: HashSet<String>,
    pub training_examples: usize,
    pub training_categories: usize,
}

const STOPWORDS: &[&str] = &[
    "a", "about", "add", "adds", "after", "all", "also", "an", "and", "any", "are", "as", "at",
    "be", "been", "before", "being", "but", "by", "can", "could", "do", "does", "doesn", "don",
    "for", "from", "get", "got", "had", "has", "have", "how", "i", "if", "in", "into", "is", "it",
    "its", "like", "make", "many", "more", "most", "my", "new", "no", "not", "of", "on", "one",
    "only", "or", "other", "our", "out", "over", "same", "should", "so", "some", "such", "t",
    "than", "that", "the", "their", "them", "then", "there", "these", "they", "this", "those",
    "to", "too", "up", "use", "used", "using", "very", "want", "was", "we", "were", "what", "when",
    "which", "who", "will", "with", "would", "x2", "xcom", "you", "your", "wotc", "war", "chosen",
    "chosen's", "chosen.", "mod", "mods", "modding", "game", "gameplay", "version", "works",
    "work", "working", "adds,", "adds.", "adding",
];

fn is_stopword(token: &str) -> bool {
    token.len() < 3 || STOPWORDS.contains(&token)
}

fn description_tokens(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    let mut current = String::new();
    for ch in text.chars().take(1500) {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                current.push(lower);
            }
        } else if !current.is_empty() {
            if current.len() >= 4 && !is_stopword(&current) && seen.insert(current.clone()) {
                result.push(current.clone());
            }
            current.clear();
            if result.len() >= MAX_DESCRIPTION_TOKENS {
                break;
            }
        }
    }
    if result.len() < MAX_DESCRIPTION_TOKENS
        && current.len() >= 4
        && !is_stopword(&current)
        && seen.insert(current.clone())
    {
        result.push(current);
    }
    result
}

pub fn features(entry: &AmlMod, cache: &WorkshopCache) -> Vec<(String, usize)> {
    let mut features = Vec::new();
    if let Some(details) = entry
        .workshop_id
        .and_then(|id| cache.items.get(&id.to_string()))
    {
        for tag in &details.tags {
            let tag = normalize_tag(tag);
            if !tag.is_empty() {
                features.push((format!("tag:{tag}"), 3));
            }
        }
    }

    let name = entry.name.as_deref().unwrap_or(&entry.id);
    let normalized_name = normalize_name(name);
    for token in normalized_name.split_whitespace() {
        if !is_stopword(token) {
            features.push((format!("name:{token}"), 2));
        }
    }

    let description = entry
        .workshop_id
        .and_then(|id| cache.items.get(&id.to_string()))
        .and_then(|details| details.description.as_deref())
        .or(entry.description.as_deref())
        .unwrap_or("");
    for token in description_tokens(description) {
        features.push((format!("desc:{token}"), 1));
    }

    features
}

pub fn train(
    feedback: &HashMap<String, CategoryFeedback>,
    settings: &AmlRoot,
    cache: &WorkshopCache,
) -> Option<Model> {
    if feedback.len() < MIN_EXAMPLES {
        return None;
    }
    let all = settings.all_mods();
    let by_id: HashMap<&str, &AmlMod> =
        all.iter().map(|entry| (entry.id.as_str(), entry)).collect();

    let mut model = Model::default();
    let mut categories = HashSet::new();
    for (mod_id, feedback) in feedback {
        let Some(entry) = by_id.get(mod_id.as_str()) else {
            continue;
        };
        let category = feedback.category.trim();
        if category.is_empty() {
            continue;
        }
        let features = features(entry, cache);
        if features.is_empty() {
            continue;
        }

        model.total_docs += 1;
        *model.doc_counts.entry(category.to_string()).or_insert(0) += 1;
        categories.insert(category.to_string());
        let category_counts = model
            .feature_counts
            .entry(category.to_string())
            .or_default();
        for (feature, weight) in features {
            *category_counts.entry(feature.clone()).or_insert(0) += weight;
            *model
                .feature_totals
                .entry(category.to_string())
                .or_insert(0) += weight;
            model.vocab.insert(feature);
        }
    }

    if model.total_docs < MIN_EXAMPLES || categories.len() < MIN_CATEGORIES {
        return None;
    }
    model.training_examples = model.total_docs;
    model.training_categories = categories.len();
    Some(model)
}

impl Model {
    pub fn predict(&self, entry: &AmlMod, cache: &WorkshopCache) -> Option<Prediction> {
        if self.total_docs == 0 {
            return None;
        }
        let features = features(entry, cache);
        if features.is_empty() {
            return None;
        }
        let vocab = self.vocab.len().max(1) as f64;
        let mut scores = Vec::with_capacity(self.doc_counts.len());
        for (category, docs) in &self.doc_counts {
            let prior = (*docs as f64 / self.total_docs as f64).ln();
            let total = *self.feature_totals.get(category).unwrap_or(&0) as f64;
            let counts = self.feature_counts.get(category);
            let mut log_likelihood = 0.0;
            for (feature, weight) in &features {
                let count = counts
                    .and_then(|map| map.get(feature))
                    .copied()
                    .unwrap_or(0) as f64;
                log_likelihood += *weight as f64 * ((count + 1.0) / (total + vocab)).ln();
            }
            scores.push((category.clone(), prior + log_likelihood));
        }

        let max = scores
            .iter()
            .map(|(_, score)| *score)
            .fold(f64::NEG_INFINITY, f64::max);
        let sum: f64 = scores.iter().map(|(_, score)| (score - max).exp()).sum();
        let mut probabilities: Vec<(String, f64)> = scores
            .into_iter()
            .map(|(category, score)| (category, (score - max).exp() / sum))
            .collect();
        probabilities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (category, probability) = probabilities.first()?.clone();
        let second = probabilities.get(1).map(|(_, value)| *value).unwrap_or(0.0);
        if probability < MIN_PROBABILITY || probability - second < MIN_MARGIN {
            return None;
        }
        Some(Prediction { category })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workshop::WorkshopDetails;
    use xcom2_launcher_core::{AmlGroup, AmlMods};

    #[test]
    fn stopwords_and_description_tokens() {
        let tokens = description_tokens("This mod adds new weapons and armor for your soldiers.");
        assert!(tokens.contains(&"weapons".to_string()));
        assert!(tokens.contains(&"armor".to_string()));
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"mod".to_string()));
    }

    #[test]
    fn few_examples_do_not_train() {
        let feedback = HashMap::new();
        let settings = AmlRoot::default();
        let cache = WorkshopCache::default();
        assert!(train(&feedback, &settings, &cache).is_none());
    }

    fn settings_with(mods: Vec<AmlMod>) -> AmlRoot {
        let mut settings = AmlRoot::default();
        settings.mods = AmlMods::default();
        let mut group = AmlGroup::default();
        group.entries = mods;
        settings.mods.entries.insert("Test".to_string(), group);
        settings
    }

    fn mod_entry(id: &str, name: &str, workshop_id: u64) -> AmlMod {
        AmlMod {
            id: id.to_string(),
            name: Some(name.to_string()),
            workshop_id: Some(workshop_id),
            ..Default::default()
        }
    }

    #[test]
    fn learns_from_tags() {
        let entries = vec![
            mod_entry("w1", "Rifle Pack One", 1),
            mod_entry("w2", "Rifle Pack Two", 2),
            mod_entry("w3", "Rifle Pack Three", 3),
            mod_entry("m1", "Map Pack One", 4),
            mod_entry("m2", "Map Pack Two", 5),
            mod_entry("m3", "Map Pack Three", 6),
        ];
        let settings = settings_with(entries);
        let mut cache = WorkshopCache::default();
        for id in 1..=3 {
            cache.items.insert(
                id.to_string(),
                WorkshopDetails {
                    workshop_id: id,
                    tags: vec!["Weapon".to_string()],
                    ..Default::default()
                },
            );
        }
        for id in 4..=6 {
            cache.items.insert(
                id.to_string(),
                WorkshopDetails {
                    workshop_id: id,
                    tags: vec!["Map".to_string()],
                    ..Default::default()
                },
            );
        }
        let mut feedback = HashMap::new();
        for id in ["w1", "w2", "w3"] {
            feedback.insert(
                id.to_string(),
                CategoryFeedback {
                    category: "Weapons & Items".to_string(),
                    updated: 0,
                },
            );
        }
        for id in ["m1", "m2", "m3"] {
            feedback.insert(
                id.to_string(),
                CategoryFeedback {
                    category: "Maps & Missions".to_string(),
                    updated: 0,
                },
            );
        }

        let model = train(&feedback, &settings, &cache).expect("model");
        assert_eq!(model.training_examples, 6);
        assert_eq!(model.training_categories, 2);

        let mut probe_cache = WorkshopCache::default();
        probe_cache.items.insert(
            "99".to_string(),
            WorkshopDetails {
                workshop_id: 99,
                tags: vec!["Weapon".to_string()],
                ..Default::default()
            },
        );
        let probe = mod_entry("w4", "Rifle Pack Four", 99);
        let prediction = model.predict(&probe, &probe_cache).expect("prediction");
        assert_eq!(prediction.category, "Weapons & Items");
    }
}
