//! Campaign generator: calls Ollama to pre-generate campaigns, validates quality, saves JSON.
//!
//! Usage: cargo run --release --bin generate_campaigns -- [OPTIONS]
//!   --count N        Number of campaigns to generate (default: 167)
//!   --threshold N    Minimum quality score 0-100 (default: 60)
//!   --output DIR     Output directory (default: campaigns/)

use scapegrace::gen::*;
use std::collections::{HashMap, HashSet};

// ── Quality validation ──

const RESERVED_COLORS: &[&str] = &["#66bb6a", "#e64545", "#ffd700", "#4dd0e1", "#ffa726"];

const GENERIC_NAMES: &[&str] = &[
    "monster", "boss", "enemy", "creature", "beast", "demon",
    "weapon", "sword", "shield", "armor", "helmet",
    "trap", "pit", "spike", "fire",
    "level", "dungeon", "room", "area", "zone",
    "the boss", "the monster", "the enemy",
    "dark lord", "evil king", "final boss",
];

const VALID_ROOTS: &[&str] = &[
    "C", "C#", "DB", "D", "D#", "EB", "E", "F", "F#", "GB",
    "G", "G#", "AB", "A", "A#", "BB", "B",
];

const VALID_SCALES: &[&str] = &[
    "ionian", "major", "dorian", "phrygian", "lydian", "mixolydian",
    "aeolian", "minor", "locrian", "pentatonic_major", "pentatonic_minor",
    "blues", "whole_tone", "chromatic",
];

// ── Hard fail validation ──

fn validate_overworld(ow: &OverworldResult) -> Result<(), String> {
    if ow.name.trim().is_empty() {
        return Err("Empty campaign name".into());
    }
    if ow.levels.len() < 5 || ow.levels.len() > 7 {
        return Err(format!("Expected 5-7 levels, got {}", ow.levels.len()));
    }
    if ow.font.is_none() || ow.font.as_ref().unwrap().trim().is_empty() {
        return Err("Missing campaign font".into());
    }
    if ow.description.trim().is_empty() {
        return Err("Empty campaign description".into());
    }

    let total_budget: i32 = ow.levels.iter().map(|l| l.budget).sum();
    if total_budget < 800 || total_budget > 1600 {
        return Err(format!("Total budget {} outside 800-1600 range", total_budget));
    }

    for (i, lv) in ow.levels.iter().enumerate() {
        if lv.name.trim().is_empty() {
            return Err(format!("Level {} has empty name", i));
        }
        if lv.description.trim().is_empty() {
            return Err(format!("Level {} '{}' has empty description", i, lv.name));
        }
        if lv.theme.trim().is_empty() {
            return Err(format!("Level {} '{}' has empty theme", i, lv.name));
        }
        if lv.budget < 50 || lv.budget > 500 {
            return Err(format!("Level {} '{}' budget {} outside 50-500", i, lv.name, lv.budget));
        }
    }

    Ok(())
}

fn validate_design(d: &Phase2Result) -> Result<(), String> {
    if d.tile_defs.len() < 2 {
        return Err(format!("Only {} tile defs, need at least 2", d.tile_defs.len()));
    }
    if d.boss.name.trim().is_empty() {
        return Err("Empty boss name".into());
    }
    if d.monster_types.is_empty() {
        return Err("No monster types".into());
    }
    for mt in &d.monster_types {
        if mt.name.trim().is_empty() {
            return Err("Monster type has empty name".into());
        }
    }
    if d.weapon.name.trim().is_empty() {
        return Err("Empty weapon name".into());
    }
    if d.armor.name.trim().is_empty() {
        return Err("Empty armor name".into());
    }

    Ok(())
}

// ── Scoring functions ──

fn score_completeness(designs: &[Phase2Result]) -> u32 {
    let mut total = 0;
    let mut present = 0;

    for d in designs {
        // mode
        total += 1;
        if d.mode.is_some() { present += 1; }
        // victory_message
        total += 1;
        if d.victory_message.as_ref().map_or(false, |s| !s.trim().is_empty()) { present += 1; }
        // defeat_message
        total += 1;
        if d.defeat_message.as_ref().map_or(false, |s| !s.trim().is_empty()) { present += 1; }
        // traps present
        total += 1;
        if d.traps.as_ref().map_or(false, |t| !t.is_empty()) { present += 1; }
        // boss description
        total += 1;
        if d.boss.description.as_ref().map_or(false, |s| !s.trim().is_empty()) { present += 1; }
        // weapon description
        total += 1;
        if d.weapon.description.as_ref().map_or(false, |s| !s.trim().is_empty()) { present += 1; }
        // armor description
        total += 1;
        if d.armor.description.as_ref().map_or(false, |s| !s.trim().is_empty()) { present += 1; }
        // monster descriptions
        for mt in &d.monster_types {
            total += 1;
            if mt.description.as_ref().map_or(false, |s| !s.trim().is_empty()) { present += 1; }
        }
    }

    if total == 0 { return 100; }
    ((present as f64 / total as f64) * 100.0).round() as u32
}

fn score_tile_variety(designs: &[Phase2Result]) -> u32 {
    let avg = designs.iter()
        .map(|d| d.tile_defs.len() as f64)
        .sum::<f64>() / designs.len().max(1) as f64;

    if avg >= 5.0 { 100 }
    else if avg >= 4.0 { 85 }
    else if avg >= 3.0 { 60 }
    else { 30 }
}

fn score_monster_variety(designs: &[Phase2Result]) -> u32 {
    let avg = designs.iter()
        .map(|d| d.monster_types.len() as f64)
        .sum::<f64>() / designs.len().max(1) as f64;

    if avg >= 3.0 { 100 }
    else if avg >= 2.0 { 70 }
    else { 30 }
}

fn parse_hex_color(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some([r, g, b])
    } else if hex.len() == 3 {
        let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
        let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
        let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
        Some([r, g, b])
    } else {
        None
    }
}

fn color_distance(c1: [u8; 3], c2: [u8; 3]) -> f64 {
    let rm = (c1[0] as f64 + c2[0] as f64) / 2.0;
    let dr = (c1[0] as i32 - c2[0] as i32) as f64;
    let dg = (c1[1] as i32 - c2[1] as i32) as f64;
    let db = (c1[2] as i32 - c2[2] as i32) as f64;
    ((2.0 + rm / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - rm) / 256.0) * db * db).sqrt()
}

fn score_color_quality(ow: &OverworldResult) -> u32 {
    let mut total_score = 0.0;
    let mut count = 0;

    for lv in &ow.levels {
        let palette = lv.palette.as_ref();
        let colors: Vec<&String> = palette.map_or(vec![], |p| p.iter().collect());

        // Palette size score
        let size_score = match colors.len() {
            0..=1 => 10.0,
            2 => 30.0,
            3 => 60.0,
            4..=6 => 100.0,
            _ => 90.0,
        };

        // Reserved color penalty
        let reserved_count = colors.iter()
            .filter(|c| RESERVED_COLORS.iter().any(|r| c.to_lowercase() == *r))
            .count();
        let reserved_penalty = reserved_count as f64 * 20.0;

        // Perceptual distance score
        let parsed: Vec<[u8; 3]> = colors.iter()
            .filter_map(|c| parse_hex_color(c))
            .collect();

        let dist_score = if parsed.len() >= 2 {
            let mut min_dist = f64::MAX;
            for i in 0..parsed.len() {
                for j in i + 1..parsed.len() {
                    let d = color_distance(parsed[i], parsed[j]);
                    if d < min_dist { min_dist = d; }
                }
            }
            (min_dist / 150.0 * 100.0).min(100.0)
        } else {
            50.0
        };

        let level_score = ((size_score + dist_score) / 2.0 - reserved_penalty).max(0.0);
        total_score += level_score;
        count += 1;
    }

    if count == 0 { return 50; }
    (total_score / count as f64).round() as u32
}

fn is_generic_name(name: &str) -> bool {
    let lower = name.to_lowercase().trim().to_string();
    GENERIC_NAMES.iter().any(|g| lower == *g)
}

fn score_single_name(name: &str) -> f64 {
    let trimmed = name.trim();
    if trimmed.is_empty() { return 0.0; }
    if is_generic_name(trimmed) { return 10.0; }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let word_count = words.len();

    // Penalize too short or too long
    let length_score = match word_count {
        1 => if trimmed.len() <= 4 { 40.0 } else { 60.0 },
        2..=4 => 100.0,
        5 => 80.0,
        _ => 60.0,
    };

    // Bonus for having at least one "interesting" word (> 5 chars)
    let has_interesting = words.iter().any(|w| w.len() > 5);
    if has_interesting { length_score } else { length_score * 0.8 }
}

fn score_name_quality(ow: &OverworldResult, designs: &[Phase2Result]) -> u32 {
    let mut scores = Vec::new();

    // Campaign name
    scores.push(score_single_name(&ow.name));

    // Level names
    for lv in &ow.levels {
        scores.push(score_single_name(&lv.name));
    }

    // Design entity names
    for d in designs {
        scores.push(score_single_name(&d.boss.name));
        for mt in &d.monster_types {
            scores.push(score_single_name(&mt.name));
        }
        scores.push(score_single_name(&d.weapon.name));
        scores.push(score_single_name(&d.armor.name));
    }

    if scores.is_empty() { return 50; }
    let avg = scores.iter().sum::<f64>() / scores.len() as f64;
    avg.round() as u32
}

fn score_single_description(desc: &str) -> f64 {
    let trimmed = desc.trim();
    if trimmed.is_empty() { return 0.0; }

    let words = trimmed.split_whitespace().count();
    let length_score = match words {
        0 => 0.0,
        1..=2 => 30.0,
        3..=15 => 100.0,
        16..=25 => 80.0,
        _ => 60.0,
    };

    // Truncation penalty
    if trimmed.ends_with("...") || trimmed.ends_with("..") {
        return length_score * 0.5;
    }

    length_score
}

fn score_description_quality(ow: &OverworldResult, designs: &[Phase2Result]) -> u32 {
    let mut scores = Vec::new();

    scores.push(score_single_description(&ow.description));
    for lv in &ow.levels {
        scores.push(score_single_description(&lv.description));
    }

    for d in designs {
        if let Some(desc) = &d.boss.description {
            scores.push(score_single_description(desc));
        }
        if let Some(desc) = &d.weapon.description {
            scores.push(score_single_description(desc));
        }
        if let Some(desc) = &d.armor.description {
            scores.push(score_single_description(desc));
        }
        if let Some(vm) = &d.victory_message {
            scores.push(score_single_description(vm));
        }
        if let Some(dm) = &d.defeat_message {
            scores.push(score_single_description(dm));
        }
    }

    if scores.is_empty() { return 50; }
    let avg = scores.iter().sum::<f64>() / scores.len() as f64;
    avg.round() as u32
}

fn score_mode_validity(designs: &[Phase2Result]) -> u32 {
    let mut total = 0;
    let mut valid = 0;

    for d in designs {
        total += 1;
        if let Some(mode) = &d.mode {
            let root_upper = mode.root.to_uppercase();
            let root_clean = root_upper.trim_end_matches(|c: char| c.is_ascii_digit());
            let scale_lower = mode.scale.to_lowercase();

            let root_ok = VALID_ROOTS.iter().any(|r| *r == root_clean);
            let scale_ok = VALID_SCALES.iter().any(|s| *s == scale_lower);

            if root_ok && scale_ok { valid += 1; }
        }
        // missing mode counts as 0 for this level
    }

    if total == 0 { return 100; }
    ((valid as f64 / total as f64) * 100.0).round() as u32
}

fn score_budget_distribution(ow: &OverworldResult) -> u32 {
    let n = ow.levels.len();
    if n < 5 { return 0; }

    let mut total_score = 0.0;
    let mut count = 0;

    // Expected ranges by position
    for (i, lv) in ow.levels.iter().enumerate() {
        let (lo, hi) = if i == n - 1 {
            // Last level = optional side quest
            (100, 200)
        } else if i == n - 2 {
            // Second to last = boss
            (250, 300)
        } else if i < 2 {
            // Early levels
            (120, 160)
        } else {
            // Middle levels
            (180, 220)
        };

        let budget = lv.budget;
        let score = if budget >= lo && budget <= hi {
            100.0
        } else {
            let dist = if budget < lo { lo - budget } else { budget - hi };
            let range = (hi - lo).max(1);
            (100.0 - (dist as f64 / range as f64) * 50.0).max(0.0)
        };

        total_score += score;
        count += 1;
    }

    (total_score / count as f64).round() as u32
}

fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let words_a: HashSet<String> = a.to_lowercase().split_whitespace().map(String::from).collect();
    let words_b: HashSet<String> = b.to_lowercase().split_whitespace().map(String::from).collect();

    if words_a.is_empty() && words_b.is_empty() { return 1.0; }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 { return 0.0; }
    intersection as f64 / union as f64
}

fn score_theme_coherence(ow: &OverworldResult) -> u32 {
    let themes: Vec<&str> = ow.levels.iter().map(|l| l.theme.as_str()).collect();
    if themes.len() < 2 { return 100; }

    let mut too_similar = 0;
    for i in 0..themes.len() {
        for j in i + 1..themes.len() {
            if jaccard_similarity(themes[i], themes[j]) > 0.5 {
                too_similar += 1;
            }
        }
    }

    (100_i32 - too_similar as i32 * 20).max(0) as u32
}

fn score_campaign(ow: &OverworldResult, designs: &[Phase2Result]) -> CampaignQuality {
    let breakdown = QualityBreakdown {
        completeness: score_completeness(designs),
        tile_variety: score_tile_variety(designs),
        monster_variety: score_monster_variety(designs),
        color_quality: score_color_quality(ow),
        name_quality: score_name_quality(ow, designs),
        description_quality: score_description_quality(ow, designs),
        mode_validity: score_mode_validity(designs),
        budget_distribution: score_budget_distribution(ow),
        theme_coherence: score_theme_coherence(ow),
    };

    // Weighted average
    let score = (
        breakdown.completeness as f64 * 0.10
        + breakdown.tile_variety as f64 * 0.10
        + breakdown.monster_variety as f64 * 0.10
        + breakdown.color_quality as f64 * 0.15
        + breakdown.name_quality as f64 * 0.15
        + breakdown.description_quality as f64 * 0.10
        + breakdown.mode_validity as f64 * 0.10
        + breakdown.budget_distribution as f64 * 0.10
        + breakdown.theme_coherence as f64 * 0.10
    ).round() as u32;

    CampaignQuality { score, breakdown }
}

// ── Cross-campaign analysis ──

#[derive(serde::Serialize)]
struct CrossCampaignReport {
    unique_campaign_names: bool,
    duplicate_campaign_names: Vec<String>,
    duplicate_boss_names: Vec<String>,
    similar_themes: Vec<(String, String, f64)>,
    global_uniqueness_score: u32,
}

fn analyze_cross_campaign(campaigns: &[BundledCampaign]) -> CrossCampaignReport {
    // Campaign name uniqueness
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for c in campaigns {
        *name_counts.entry(c.overworld.name.to_lowercase()).or_insert(0) += 1;
    }
    let duplicate_campaign_names: Vec<String> = name_counts.iter()
        .filter(|(_, count)| **count > 1)
        .map(|(name, _)| name.clone())
        .collect();

    // Boss name uniqueness
    let mut boss_counts: HashMap<String, usize> = HashMap::new();
    for c in campaigns {
        for d in &c.designs {
            *boss_counts.entry(d.boss.name.to_lowercase()).or_insert(0) += 1;
        }
    }
    let duplicate_boss_names: Vec<String> = boss_counts.iter()
        .filter(|(_, count)| **count > 1)
        .map(|(name, _)| name.clone())
        .collect();

    // Theme similarity across campaigns
    let mut all_themes: Vec<(String, String)> = Vec::new(); // (campaign_name, theme)
    for c in campaigns {
        for lv in &c.overworld.levels {
            all_themes.push((c.overworld.name.clone(), lv.theme.clone()));
        }
    }

    let mut similar_themes = Vec::new();
    for i in 0..all_themes.len() {
        for j in i + 1..all_themes.len() {
            // Only flag cross-campaign similarities
            if all_themes[i].0 == all_themes[j].0 { continue; }
            let sim = jaccard_similarity(&all_themes[i].1, &all_themes[j].1);
            if sim > 0.7 {
                similar_themes.push((all_themes[i].1.clone(), all_themes[j].1.clone(), sim));
            }
        }
    }

    let total_items = campaigns.len() + boss_counts.len();
    let unique_items = name_counts.values().filter(|v| **v == 1).count()
        + boss_counts.values().filter(|v| **v == 1).count();
    let global_uniqueness_score = if total_items > 0 {
        ((unique_items as f64 / total_items as f64) * 100.0).round() as u32
    } else {
        100
    };

    CrossCampaignReport {
        unique_campaign_names: duplicate_campaign_names.is_empty(),
        duplicate_campaign_names,
        duplicate_boss_names,
        similar_themes,
        global_uniqueness_score,
    }
}

// ── Quality report ──

#[derive(serde::Serialize)]
struct QualityReport {
    generated_at: String,
    total_campaigns: usize,
    total_levels: usize,
    rejected_overworlds: usize,
    rejected_designs: usize,
    full_regenerations: usize,
    score_distribution: ScoreDistribution,
    campaigns: Vec<CampaignReportEntry>,
    cross_campaign: CrossCampaignReport,
}

#[derive(serde::Serialize)]
struct ScoreDistribution {
    min: u32,
    max: u32,
    mean: f64,
    median: u32,
    p10: u32,
    p90: u32,
}

#[derive(serde::Serialize)]
struct CampaignReportEntry {
    id: usize,
    name: String,
    score: u32,
    breakdown: QualityBreakdown,
    levels: Vec<LevelReportEntry>,
    retries: RetryInfo,
}

#[derive(serde::Serialize)]
struct LevelReportEntry {
    name: String,
    boss: String,
    monster_types: usize,
    tile_defs: usize,
}

#[derive(serde::Serialize, Default)]
struct RetryInfo {
    overworld: usize,
    levels: Vec<usize>,
}

fn compute_distribution(scores: &[u32]) -> ScoreDistribution {
    let mut sorted = scores.to_vec();
    sorted.sort();
    let n = sorted.len();

    ScoreDistribution {
        min: sorted.first().copied().unwrap_or(0),
        max: sorted.last().copied().unwrap_or(0),
        mean: (scores.iter().sum::<u32>() as f64 / n.max(1) as f64 * 10.0).round() / 10.0,
        median: sorted.get(n / 2).copied().unwrap_or(0),
        p10: sorted.get(n / 10).copied().unwrap_or(0),
        p90: sorted.get(n * 9 / 10).copied().unwrap_or(0),
    }
}

// ── LLM interaction ──

fn generate_overworld_raw(client: &reqwest::blocking::Client, api_key: &str, model: &str, theme: Option<&str>) -> Result<OverworldResult, String> {
    let prompt = build_overworld_prompt_themed(theme);
    let content = call_llm_streaming::<fn()>(client, api_key, model, &prompt, None)?;
    let result: OverworldResult = serde_json::from_str(&content)
        .map_err(|e| format!("Overworld parse error: {}\n\nRaw: {}", e, &content[..content.len().min(300)]))?;
    Ok(result)
}

fn generate_design_raw(
    client: &reqwest::blocking::Client, api_key: &str, model: &str,
    campaign_name: &str, campaign_desc: &str, config: &LevelConfig, theme: Option<&str>,
) -> Result<Phase2Result, String> {
    let prompt = build_single_level_design_prompt_themed(campaign_name, campaign_desc, config, theme);
    let content = call_llm_streaming::<fn()>(client, api_key, model, &prompt, None)?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Design parse error: {}\n\nRaw: {}", e, &content[..content.len().min(300)]))
}

// ── Main ──

fn main() {
    dotenvy::dotenv().ok();

    let args: Vec<String> = std::env::args().collect();
    let mut count: usize = 167;
    let mut threshold: u32 = 60;
    let mut output_file = "campaigns.json".to_string();
    let mut theme: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--count" => { i += 1; count = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(167); }
            "--threshold" => { i += 1; threshold = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(60); }
            "-o" | "--output" => { i += 1; output_file = args.get(i).cloned().unwrap_or("campaigns.json".into()); }
            "--theme" => { i += 1; theme = args.get(i).cloned(); }
            "--help" | "-h" => {
                eprintln!("Usage: generate_campaigns [OPTIONS]");
                eprintln!("  --count N        Number of campaigns (default: 167)");
                eprintln!("  --threshold N    Min quality score 0-100 (default: 60)");
                eprintln!("  -o, --output F   Output file (default: campaigns.json)");
                eprintln!("  --theme TEXT     Theme for all campaigns (e.g. \"hitchhiker's guide\")");
                std::process::exit(0);
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let api_key = llm_api_key();
    let model = llm_model();
    let base_url = llm_base_url();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("Failed to create HTTP client");

    eprintln!("=== Campaign Generator ===");
    eprintln!("Model: {}", model);
    eprintln!("Base URL: {}", base_url);
    eprintln!("Target: {} campaigns ({} levels)", count, count * 6);
    eprintln!("Quality threshold: {}", threshold);
    eprintln!("Output: {}", output_file);
    if let Some(t) = &theme {
        eprintln!("Theme: {}", t);
    }
    eprintln!();

    let mut all_campaigns: Vec<BundledCampaign> = Vec::new();
    let mut total_rejected_overworlds: usize = 0;
    let mut total_rejected_designs: usize = 0;
    let mut total_full_regenerations: usize = 0;
    let mut campaign_reports: Vec<CampaignReportEntry> = Vec::new();

    for campaign_idx in 0..count {
        let mut campaign_ok = false;

        for full_retry in 0..3 {
            if full_retry > 0 {
                total_full_regenerations += 1;
                eprintln!("  !! Full regeneration attempt {}/2", full_retry);
            }

            let mut retry_info = RetryInfo::default();

            // Generate overworld
            eprint!("[{}/{}] Generating overworld... ", campaign_idx + 1, count);
            let overworld = loop {
                match generate_overworld_raw(&client, &api_key, &model, theme.as_deref()) {
                    Ok(ow) => match validate_overworld(&ow) {
                        Ok(()) => {
                            eprintln!("\"{}\" ({} levels)", ow.name, ow.levels.len());
                            break ow;
                        }
                        Err(e) => {
                            retry_info.overworld += 1;
                            total_rejected_overworlds += 1;
                            eprintln!("REJECTED: {} (retry {})", e, retry_info.overworld);
                            if retry_info.overworld >= 3 {
                                eprintln!("  !! 3 overworld failures, skipping campaign");
                                break ow; // will fail quality check
                            }
                            eprint!("[{}/{}] Retrying overworld... ", campaign_idx + 1, count);
                        }
                    }
                    Err(e) => {
                        retry_info.overworld += 1;
                        total_rejected_overworlds += 1;
                        eprintln!("ERROR: {} (retry {})", e, retry_info.overworld);
                        if retry_info.overworld >= 3 {
                            eprintln!("  !! 3 overworld failures, aborting this campaign");
                            break OverworldResult {
                                name: String::new(), font: None, description_font: None,
                                label_font: None, description: String::new(),
                                bg_color: None, text_color: None, levels: vec![], store: None,
                                boss_level: None,
                                connections: None,
                                node_positions: None,
                                bg_image: None,
                                bg_gradient: None,
                                bg_mode: None,
                                terrain_seed: None,
                                bg_prompt: None,
                                ow_region_offsets: None,
                            };
                        }
                        eprint!("[{}/{}] Retrying overworld... ", campaign_idx + 1, count);
                    }
                }
            };

            if overworld.levels.is_empty() {
                continue; // full retry
            }

            // Generate designs for each level
            let mut designs: Vec<Phase2Result> = Vec::new();
            let mut level_retries: Vec<usize> = Vec::new();
            let mut all_designs_ok = true;

            for (li, lv) in overworld.levels.iter().enumerate() {
                let config = LevelConfig {
                    title: lv.name.clone(),
                    font: lv.font.clone().unwrap_or_else(|| overworld.font.clone().unwrap_or_default()),
                    description: lv.description.clone(),
                    theme: lv.theme.clone(),
                    palette: lv.palette.clone().unwrap_or_default(),
                    budget: lv.budget,
                    floor: li as i32 + 1,
                    campaign_tier: 0,
                };

                eprint!("  Level {}/{}: \"{}\" ... ", li + 1, overworld.levels.len(), lv.name);
                let mut retries = 0;
                let design = loop {
                    match generate_design_raw(&client, &api_key, &model, &overworld.name, &overworld.description, &config, theme.as_deref()) {
                        Ok(d) => match validate_design(&d) {
                            Ok(()) => {
                                eprintln!("boss \"{}\" ({} monsters, {} tiles) OK",
                                    d.boss.name, d.monster_types.len(), d.tile_defs.len());
                                break Some(d);
                            }
                            Err(e) => {
                                retries += 1;
                                total_rejected_designs += 1;
                                eprintln!("REJECTED: {} (retry {})", e, retries);
                                if retries >= 3 {
                                    eprintln!("    !! 3 design failures for level '{}'", lv.name);
                                    break None;
                                }
                                eprint!("  Level {}/{}: \"{}\" retry... ", li + 1, overworld.levels.len(), lv.name);
                            }
                        }
                        Err(e) => {
                            retries += 1;
                            total_rejected_designs += 1;
                            eprintln!("ERROR: {} (retry {})", e, retries);
                            if retries >= 3 {
                                eprintln!("    !! 3 design failures for level '{}'", lv.name);
                                break None;
                            }
                            eprint!("  Level {}/{}: \"{}\" retry... ", li + 1, overworld.levels.len(), lv.name);
                        }
                    }
                };

                level_retries.push(retries);
                match design {
                    Some(d) => designs.push(d),
                    None => { all_designs_ok = false; break; }
                }
            }

            retry_info.levels = level_retries;

            if !all_designs_ok {
                continue; // full retry
            }

            // Score the campaign
            let quality = score_campaign(&overworld, &designs);
            eprint!("  Campaign score: {} ", quality.score);

            if quality.score < threshold {
                eprintln!("BELOW THRESHOLD ({})", threshold);
                continue; // full retry
            }

            eprintln!("OK");

            // Build report entry
            let level_reports: Vec<LevelReportEntry> = overworld.levels.iter().zip(designs.iter())
                .map(|(lv, d)| LevelReportEntry {
                    name: lv.name.clone(),
                    boss: d.boss.name.clone(),
                    monster_types: d.monster_types.len(),
                    tile_defs: d.tile_defs.len(),
                })
                .collect();

            campaign_reports.push(CampaignReportEntry {
                id: campaign_idx,
                name: overworld.name.clone(),
                score: quality.score,
                breakdown: quality.breakdown.clone(),
                levels: level_reports,
                retries: retry_info,
            });

            // Store campaign
            let campaign = BundledCampaign { id: uuid_v4(), overworld, designs, quality, settings: CampaignSettings::default(), monster_templates: None };
            all_campaigns.push(campaign);
            campaign_ok = true;
            break;
        }

        if !campaign_ok {
            eprintln!("  !! FAILED to generate campaign {} after 3 full attempts, skipping", campaign_idx + 1);
        }

        eprintln!();
    }

    // Cross-campaign analysis
    eprintln!("=== Cross-Campaign Analysis ===");
    let cross = analyze_cross_campaign(&all_campaigns);

    if cross.unique_campaign_names {
        eprintln!("Campaign names: all unique");
    } else {
        eprintln!("Duplicate campaign names: {:?}", cross.duplicate_campaign_names);
    }

    if cross.duplicate_boss_names.is_empty() {
        eprintln!("Boss names: all unique");
    } else {
        eprintln!("Duplicate boss names: {} duplicates", cross.duplicate_boss_names.len());
    }

    if cross.similar_themes.is_empty() {
        eprintln!("Theme similarity: no highly similar themes");
    } else {
        eprintln!("Similar themes: {} pairs with >70% similarity", cross.similar_themes.len());
    }

    eprintln!("Global uniqueness score: {}", cross.global_uniqueness_score);

    // Write the pack file
    let pack = BundledPack {
        theme: theme.clone(),
        campaigns: all_campaigns,
        strings: PackStrings::default(),
        item_sprites: Default::default(),
        item_names: Default::default(),
        item_descriptions: Default::default(),
    };
    let json = serde_json::to_string(&pack).expect("Failed to serialize pack");
    std::fs::write(&output_file, &json).expect("Failed to write pack file");

    // Write quality report alongside
    let scores: Vec<u32> = campaign_reports.iter().map(|c| c.score).collect();
    let distribution = compute_distribution(&scores);

    let report = QualityReport {
        generated_at: chrono_now(),
        total_campaigns: pack.campaigns.len(),
        total_levels: pack.campaigns.len() * 6,
        rejected_overworlds: total_rejected_overworlds,
        rejected_designs: total_rejected_designs,
        full_regenerations: total_full_regenerations,
        score_distribution: distribution,
        campaigns: campaign_reports,
        cross_campaign: cross,
    };

    let report_path = output_file.replace(".json", "_report.json");
    let report_json = serde_json::to_string_pretty(&report).expect("Failed to serialize report");
    std::fs::write(&report_path, &report_json).expect("Failed to write report");

    eprintln!();
    eprintln!("=== Summary ===");
    eprintln!("Generated: {}/{} campaigns ({} levels)", pack.campaigns.len(), count, pack.campaigns.len() * 6);
    eprintln!("Score range: {}-{}, mean: {:.1}",
        report.score_distribution.min, report.score_distribution.max, report.score_distribution.mean);
    eprintln!("Rejected: {} overworlds, {} designs, {} full regenerations",
        total_rejected_overworlds, total_rejected_designs, total_full_regenerations);
    eprintln!("Pack: {} ({:.1} MB)", output_file, json.len() as f64 / 1_000_000.0);
    eprintln!("Report: {}", report_path);
}

fn uuid_v4() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 1
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11],
        bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

fn chrono_now() -> String {
    // Simple ISO 8601 without chrono crate
    use std::time::SystemTime;
    let d = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    let secs = d.as_secs();
    // Good enough for a report timestamp
    format!("{}s_since_epoch", secs)
}
