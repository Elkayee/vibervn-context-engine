//! Normalization and deduplication helpers for settings.

use std::collections::{HashMap, HashSet};

use crate::config::Settings;
use crate::store::normalize_repo_path;

/// Normalize repo paths in `Settings.repos` and keys in `Settings.repo_generations`.
///
/// Idempotently deduplicates `repos` while preserving the first logical order.
/// Canonicalizes `repo_generations` keys; when aliases collide, the highest
/// generation counter is retained so a retired generation is never revived.
/// Generation entries for repos not currently present in `repos` are preserved.
///
/// Returns `true` if any field in `settings` was modified.
pub fn normalize_and_dedup_settings(settings: &mut Settings) -> bool {
    let mut changed = false;

    // 1. Deduplicate and normalize repos preserving first logical order.
    let original_repos = settings.repos.clone();
    let mut seen = HashSet::new();
    let mut deduplicated_repos = Vec::with_capacity(settings.repos.len());
    for repo in &settings.repos {
        let normalized = normalize_repo_path(repo);
        if seen.insert(normalized.clone()) {
            deduplicated_repos.push(normalized);
        }
    }
    if deduplicated_repos != original_repos {
        settings.repos = deduplicated_repos;
        changed = true;
    }

    // 2. Normalize repo_generations keys, merging collisions by taking max(counter).
    let original_generations = settings.repo_generations.clone();
    let mut normalized_generations: HashMap<String, u32> =
        HashMap::with_capacity(settings.repo_generations.len());
    for (k, v) in &settings.repo_generations {
        let norm_k = normalize_repo_path(k);
        normalized_generations
            .entry(norm_k)
            .and_modify(|existing| *existing = (*existing).max(*v))
            .or_insert(*v);
    }
    if normalized_generations != original_generations {
        settings.repo_generations = normalized_generations;
        changed = true;
    }

    changed
}
