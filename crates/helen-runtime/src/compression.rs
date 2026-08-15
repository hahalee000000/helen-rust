//! Compression (Task 8.3) — port of `helen/runtime/{graduated_compression,
//! cache_aware_compression, reactive_compaction}.py`.
//!
//! Five-layer graduated pipeline ("cheapest move first"), cache-aware
//! compression for prompt-cache hit rate, and mid-turn reactive compaction.

use crate::transcript::Message;

// ---------------------------------------------------------------------------
// Thresholds and constants
// ---------------------------------------------------------------------------

/// Phase 2: Compression thresholds (Python parity).
pub const COMPRESSION_THRESHOLDS: &[(&str, f64)] = &[
    ("budget_reduction", 0.60),
    ("snip", 0.70),
    ("microcompact", 0.80),
    ("context_collapse", 0.90),
    ("auto_compact", 0.95),
];

pub fn compression_threshold(name: &str) -> f64 {
    COMPRESSION_THRESHOLDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v)
        .unwrap_or(1.0)
}

pub const LAYER_NONE: &str = "none";
pub const LAYER_BUDGET_REDUCTION: &str = "budget_reduction";
pub const LAYER_SNIP: &str = "snip";
pub const LAYER_MICROCOMPACT: &str = "microcompact";
pub const LAYER_CONTEXT_COLLAPSE: &str = "context_collapse";
pub const LAYER_AUTO_COMPACT: &str = "auto_compact";

pub const BUDGET_REDUCTION_MAX_CHARS: usize = 4000;
pub const SNIP_KEEP_RECENT: usize = 8;
pub const MICROCOMPACT_KEEP_RECENT: usize = 5;
pub const CONTEXT_COLLAPSE_THRESHOLD: usize = 20;

/// Cache-aware constants.
pub const DEFAULT_CACHE_ZONE_RATIO: f64 = 0.30;
pub const MIN_CACHE_ZONE_MESSAGES: usize = 5;
pub const BATCH_COMPRESSION_THRESHOLD: f64 = 0.75;
pub const CACHE_HIT_STABLE: &str = "stable";
pub const CACHE_HIT_PARTIAL: &str = "partial";
pub const CACHE_HIT_INVALIDATED: &str = "invalidated";

/// Reactive compaction constants.
pub const STRUCTURAL_THRESHOLD: f64 = 0.90;
pub const SEMANTIC_THRESHOLD: f64 = 0.95;
pub const PRESERVE_RECENT: usize = 4;
pub const STRUCTURAL_BLOCK_SIZE: usize = 10;

// ---------------------------------------------------------------------------
// Helper: usage ratio
// ---------------------------------------------------------------------------

pub fn calculate_usage_ratio(history: &[Message], max_tokens: usize) -> f64 {
    if history.is_empty() || max_tokens == 0 {
        return 0.0;
    }
    let total_tokens: usize = history.iter().map(|m| m.token_count()).sum();
    total_tokens as f64 / max_tokens as f64
}

/// Extract file references from content (Python regex parity).
fn extract_file_refs(text: &str) -> Vec<String> {
    let mut refs: Vec<String> = Vec::new();
    let re = regex::Regex::new(r"[\w./-]+\.(?:py|js|ts|json|yaml|yml|md|txt|helen|rs|go|java|c|cpp|h|hpp)").unwrap();
    for caps in re.captures_iter(text) {
        let s = caps[0].to_string();
        if !refs.contains(&s) {
            refs.push(s);
        }
    }
    refs
}

// ---------------------------------------------------------------------------
// Layer 1: Budget Reduction
// ---------------------------------------------------------------------------

fn budget_reduction(history: &[Message]) -> Vec<Message> {
    let mut result = Vec::with_capacity(history.len());
    for msg in history {
        if msg.pinned {
            result.push(msg.clone());
            continue;
        }
        let content_text = crate::transcript::message_text_parts(&msg.content).0;
        if msg.role == "tool" && content_text.len() > BUDGET_REDUCTION_MAX_CHARS {
            let tool_id = msg.tool_call_id.clone().unwrap_or_else(|| "unknown".to_string());
            let preview: String = content_text.chars().take(200).collect();
            let mut copy = msg.clone();
            copy.content = serde_json::Value::String(format!(
                "[Tool result cleared: {tool_id}, {} chars]\nPreview: {preview}...",
                content_text.len()
            ));
            copy.compressed = true;
            result.push(copy);
        } else {
            result.push(msg.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Layer 2: Snip
// ---------------------------------------------------------------------------

fn snip(history: &[Message], keep_recent: usize) -> Vec<Message> {
    if history.len() <= keep_recent {
        return history.to_vec();
    }
    let pinned_msgs: Vec<&Message> = history.iter().filter(|m| m.pinned).collect();
    let system_msgs: Vec<&Message> = history.iter().filter(|m| m.role == "system").collect();
    let conversation_msgs: Vec<&Message> = history
        .iter()
        .filter(|m| m.role != "system" && !m.pinned)
        .collect();

    if conversation_msgs.len() <= keep_recent {
        return history.to_vec();
    }
    let recent = &conversation_msgs[conversation_msgs.len() - keep_recent..];

    // Reconstruct preserving original order (Python: keep_set by id).
    let keep: Vec<&Message> = system_msgs
        .iter()
        .chain(pinned_msgs.iter())
        .chain(recent.iter())
        .copied()
        .collect();
    history
        .iter()
        .filter(|m| keep.iter().any(|k| std::ptr::eq(*k, *m)))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Layer 3: Microcompact
// ---------------------------------------------------------------------------

fn microcompact(history: &[Message], keep_recent: usize) -> Vec<Message> {
    let mut tool_result_indices: Vec<usize> = Vec::new();
    let mut pinned_indices: Vec<usize> = Vec::new();
    for (i, msg) in history.iter().enumerate() {
        if msg.role == "tool" {
            tool_result_indices.push(i);
            if msg.pinned {
                pinned_indices.push(i);
            }
        }
    }
    if tool_result_indices.len() <= keep_recent {
        return history.to_vec();
    }

    let mut result = history.to_vec();
    let clear_count = tool_result_indices.len() - keep_recent;
    for idx in tool_result_indices.iter().take(clear_count) {
        if pinned_indices.contains(idx) {
            continue;
        }
        let msg = &result[*idx];
        if !msg.compressed {
            let tool_id = msg.tool_call_id.clone().unwrap_or_else(|| "unknown".to_string());
            let mut copy = msg.clone();
            copy.content = serde_json::Value::String(format!("[Tool result cleared: {tool_id}]"));
            copy.compressed = true;
            copy.pinned = true; // Preserve pinned status in copy (Python quirk)
            result[*idx] = copy;
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Layer 4: Context Collapse (temporal)
// ---------------------------------------------------------------------------

fn context_collapse(history: &[Message]) -> Vec<Message> {
    if history.len() <= CONTEXT_COLLAPSE_THRESHOLD {
        return history.to_vec();
    }
    let cutoff = history.len() - CONTEXT_COLLAPSE_THRESHOLD;
    let system_msgs: Vec<&Message> = history.iter().filter(|m| m.role == "system").collect();
    let pinned_old: Vec<&Message> = history
        .iter()
        .enumerate()
        .filter(|(i, m)| *i < cutoff && m.pinned)
        .map(|(_, m)| m)
        .collect();
    let old_msgs: Vec<&Message> = history
        .iter()
        .enumerate()
        .filter(|(i, m)| *i < cutoff && m.role != "system" && !m.pinned)
        .map(|(_, m)| m)
        .collect();
    let recent_msgs: Vec<&Message> = history
        .iter()
        .enumerate()
        .filter(|(i, m)| *i >= cutoff && m.role != "system")
        .map(|(_, m)| m)
        .collect();

    if old_msgs.is_empty() {
        return history.to_vec();
    }

    let mut timeline_parts = vec![format!("[Context Collapse: {} turns archived as timeline]", old_msgs.len())];

    let block_size = 10usize;
    for i in (0..old_msgs.len()).step_by(block_size) {
        let end = (i + block_size).min(old_msgs.len());
        let block = &old_msgs[i..end];
        if let Some(s) = summarize_block(block, i, end) {
            timeline_parts.push(s);
        }
    }

    if let Some(stats) = extract_global_stats(&old_msgs) {
        timeline_parts.push(stats);
    }

    timeline_parts.push(format!("[Preserved: last {} turns for continuity]", recent_msgs.len()));
    let summary_text = timeline_parts.join("\n");

    let summary_msg = Message::new(
        "system",
        serde_json::Value::String(summary_text),
        Vec::new(),
        None,
        String::new(),
        Some("system".to_string()),
        100,
        false,
        false,
        None,
        String::new(),
        String::new(),
        Vec::new(),
    );

    let mut result: Vec<Message> = system_msgs.iter().map(|m| (*m).clone()).collect();
    result.extend(pinned_old.iter().map(|m| (*m).clone()));
    result.push(summary_msg);
    result.extend(recent_msgs.iter().map(|m| (*m).clone()));
    result
}

fn summarize_block(block: &[&Message], start_idx: usize, end_idx: usize) -> Option<String> {
    let mut parts = vec![format!("  [{start_idx}-{end_idx}]")];

    let mut file_refs: Vec<String> = Vec::new();
    for msg in block {
        let text = crate::transcript::message_text_parts(&msg.content).0;
        for r in extract_file_refs(&text) {
            if !file_refs.contains(&r) {
                file_refs.push(r);
            }
        }
    }
    if !file_refs.is_empty() {
        file_refs.sort();
        let mut top: Vec<&str> = file_refs.iter().map(|s| s.as_str()).take(3).collect();
        top.sort_unstable();
        parts.push(format!("Files: {}", top.join(", ")));
    }

    let mut tool_counts: Vec<(String, usize)> = Vec::new();
    for msg in block {
        if msg.role == "assistant" && !msg.tool_calls.is_empty() {
            for tc in &msg.tool_calls {
                let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                if let Some(e) = tool_counts.iter_mut().find(|(n, _)| *n == name) {
                    e.1 += 1;
                } else {
                    tool_counts.push((name, 1));
                }
            }
        }
    }
    if !tool_counts.is_empty() {
        tool_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
        let mut top: Vec<String> = tool_counts.iter().take(3).map(|(n, c)| format!("{n}({c})")).collect();
        top.sort();
        parts.push(format!("Tools: {}", top.join(", ")));
    }

    let mut user_intents: Vec<String> = Vec::new();
    for msg in block {
        if msg.role == "user" {
            let text = crate::transcript::message_text_parts(&msg.content).0;
            let first_line: String = text.split('\n').next().unwrap_or("").chars().take(60).collect::<String>().trim().to_string();
            if !first_line.is_empty() && !user_intents.contains(&first_line) {
                user_intents.push(first_line);
            }
        }
    }
    if !user_intents.is_empty() {
        parts.push(format!("Tasks: {}", user_intents.iter().take(2).cloned().collect::<Vec<_>>().join("; ")));
    }

    if parts.len() > 1 {
        Some(parts.join(" | "))
    } else {
        None
    }
}

fn extract_global_stats(old_msgs: &[&Message]) -> Option<String> {
    let mut stats_parts = vec!["[Global]".to_string()];

    let user_turns = old_msgs.iter().filter(|m| m.role == "user").count();
    let assistant_turns = old_msgs.iter().filter(|m| m.role == "assistant").count();
    stats_parts.push(format!("Turns: {user_turns}u/{assistant_turns}a"));

    let total_tools: usize = old_msgs
        .iter()
        .filter(|m| m.role == "assistant" && !m.tool_calls.is_empty())
        .map(|m| m.tool_calls.len())
        .sum();
    if total_tools > 0 {
        stats_parts.push(format!("Tool calls: {total_tools}"));
    }

    let errors = old_msgs
        .iter()
        .filter(|m| {
            m.role == "tool" && crate::transcript::message_text_parts(&m.content).0.to_lowercase().contains("error")
        })
        .count();
    if errors > 0 {
        stats_parts.push(format!("Errors: {errors}"));
    }

    if stats_parts.len() > 1 {
        Some(stats_parts.join(" "))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Layer 5: Auto-Compact
// ---------------------------------------------------------------------------

fn auto_compact(history: &[Message], keep_recent: usize, target_tokens: usize) -> Vec<Message> {
    if history.len() <= keep_recent + 2 {
        return history.to_vec();
    }

    let system_msgs: Vec<&Message> = history.iter().filter(|m| m.role == "system").collect();
    let cutoff = history.len() - keep_recent;
    let pinned_old: Vec<&Message> = history
        .iter()
        .enumerate()
        .filter(|(i, m)| *i < cutoff && m.pinned)
        .map(|(_, m)| m)
        .collect();
    let conversation_msgs: Vec<&Message> = history
        .iter()
        .enumerate()
        .filter(|(i, m)| m.role != "system" && !(*i < cutoff && m.pinned))
        .map(|(_, m)| m)
        .collect();

    if conversation_msgs.len() <= keep_recent {
        return history.to_vec();
    }

    let old_msgs = &conversation_msgs[..conversation_msgs.len() - keep_recent];
    let recent_msgs = &conversation_msgs[conversation_msgs.len() - keep_recent..];

    // No LLM client in the base port — structural fallback (Python behavior
    // when llm_client is None). LLM path arrives with the M5 http client.
    let _ = target_tokens;
    structural_auto_compact(&system_msgs, &pinned_old, old_msgs, recent_msgs)
}

fn structural_auto_compact(
    system_msgs: &[&Message],
    pinned_old: &[&Message],
    old_msgs: &[&Message],
    recent_msgs: &[&Message],
) -> Vec<Message> {
    let mut summary_parts = vec![format!("[Auto-Compact: {} turns archived]", old_msgs.len())];

    let mut file_refs: Vec<String> = Vec::new();
    for msg in old_msgs {
        let text = crate::transcript::message_text_parts(&msg.content).0;
        for r in extract_file_refs(&text) {
            if !file_refs.contains(&r) {
                file_refs.push(r);
            }
        }
    }
    if !file_refs.is_empty() {
        file_refs.sort();
        let mut top: Vec<&str> = file_refs.iter().map(|s| s.as_str()).take(8).collect();
        top.sort_unstable();
        summary_parts.push(format!("Files: {}", top.join(", ")));
    }

    let mut tool_counts: Vec<(String, usize)> = Vec::new();
    for msg in old_msgs {
        if msg.role == "assistant" && !msg.tool_calls.is_empty() {
            for tc in &msg.tool_calls {
                let name = tc.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                if let Some(e) = tool_counts.iter_mut().find(|(n, _)| *n == name) {
                    e.1 += 1;
                } else {
                    tool_counts.push((name, 1));
                }
            }
        }
    }
    if !tool_counts.is_empty() {
        tool_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
        let mut top: Vec<String> = tool_counts.iter().take(5).map(|(n, c)| format!("{n}({c})")).collect();
        top.sort();
        summary_parts.push(format!("Tools: {}", top.join(", ")));
    }

    let mut user_intents: Vec<String> = Vec::new();
    for msg in old_msgs {
        if msg.role == "user" {
            let text = crate::transcript::message_text_parts(&msg.content).0;
            let first_line: String = text.split('\n').next().unwrap_or("").chars().take(100).collect::<String>().trim().to_string();
            if !first_line.is_empty() && !user_intents.contains(&first_line) {
                user_intents.push(first_line);
            }
        }
    }
    if !user_intents.is_empty() {
        summary_parts.push(format!("Tasks: {}", user_intents.iter().take(3).cloned().collect::<Vec<_>>().join("; ")));
    }

    let errors = old_msgs
        .iter()
        .filter(|m| {
            m.role == "tool" && crate::transcript::message_text_parts(&m.content).0.to_lowercase().contains("error")
        })
        .count();
    if errors > 0 {
        summary_parts.push(format!("Errors encountered: {errors}"));
    }

    let mut summary_text = summary_parts.join("\n");
    summary_text += &format!("\n[Preserved: last {} turns]", recent_msgs.len());

    let summary_msg = Message::new(
        "system",
        serde_json::Value::String(summary_text),
        Vec::new(),
        None,
        String::new(),
        Some("system".to_string()),
        100,
        false,
        false,
        None,
        String::new(),
        String::new(),
        Vec::new(),
    );

    let mut result: Vec<Message> = system_msgs.iter().map(|m| (*m).clone()).collect();
    result.extend(pinned_old.iter().map(|m| (*m).clone()));
    result.push(summary_msg);
    result.extend(recent_msgs.iter().map(|m| (*m).clone()));
    result
}

// ---------------------------------------------------------------------------
// Graduated pipeline entry
// ---------------------------------------------------------------------------

/// Five-layer graduated compression pipeline — cheapest move first.
///
/// Returns `(compressed_history, layer_used)`.
pub fn graduated_compress(
    history: &[Message],
    usage_ratio: f64,
    max_tokens: Option<usize>,
) -> (Vec<Message>, String) {
    let max_tokens = max_tokens.unwrap_or(crate::token::DEFAULT_CONTEXT_WINDOW);
    if history.is_empty() {
        return (history.to_vec(), LAYER_NONE.to_string());
    }

    let mut current_history = history.to_vec();
    let mut current_ratio = usage_ratio;
    let mut last_layer = LAYER_NONE.to_string();

    // Layer 1: Budget Reduction (60%)
    if current_ratio >= compression_threshold(LAYER_BUDGET_REDUCTION) {
        let new_history = budget_reduction(&current_history);
        let new_ratio = calculate_usage_ratio(&new_history, max_tokens);
        if new_ratio < current_ratio {
            current_history = new_history;
            current_ratio = new_ratio;
            last_layer = LAYER_BUDGET_REDUCTION.to_string();
        }
    }

    // Layer 2: Snip (70%)
    if current_ratio >= compression_threshold(LAYER_SNIP) {
        let new_history = snip(&current_history, SNIP_KEEP_RECENT);
        let new_ratio = calculate_usage_ratio(&new_history, max_tokens);
        if new_ratio < current_ratio {
            current_history = new_history;
            current_ratio = new_ratio;
            last_layer = LAYER_SNIP.to_string();
        }
    }

    // Layer 3: Microcompact (80%)
    if current_ratio >= compression_threshold(LAYER_MICROCOMPACT) {
        let new_history = microcompact(&current_history, MICROCOMPACT_KEEP_RECENT);
        let new_ratio = calculate_usage_ratio(&new_history, max_tokens);
        if new_ratio < current_ratio {
            current_history = new_history;
            current_ratio = new_ratio;
            last_layer = LAYER_MICROCOMPACT.to_string();
        }
    }

    // Layer 4: Context Collapse (90%)
    if current_ratio >= compression_threshold(LAYER_CONTEXT_COLLAPSE) {
        let new_history = context_collapse(&current_history);
        let new_ratio = calculate_usage_ratio(&new_history, max_tokens);
        if new_ratio < current_ratio {
            current_history = new_history;
            current_ratio = new_ratio;
            last_layer = LAYER_CONTEXT_COLLAPSE.to_string();
        }
    }

    // Layer 5: Auto-Compact (95%) — structural fallback (no LLM in base port)
    if current_ratio >= compression_threshold(LAYER_AUTO_COMPACT) {
        let new_history = auto_compact(&current_history, 4, max_tokens / 10);
        let new_ratio = calculate_usage_ratio(&new_history, max_tokens);
        if new_ratio < current_ratio {
            current_history = new_history;
            last_layer = LAYER_AUTO_COMPACT.to_string();
            let _ = &new_ratio;
        } else {
            // Last resort: aggressive snip keeping only last 4 messages
            if current_history.len() > 4 {
                let system_msgs: Vec<Message> = current_history
                    .iter()
                    .filter(|m| m.role == "system")
                    .cloned()
                    .collect();
                let recent: Vec<Message> = current_history
                    .iter()
                    .filter(|m| m.role != "system")
                    .rev()
                    .take(4)
                    .cloned()
                    .collect::<Vec<Message>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<Message>>();
                current_history = system_msgs;
                current_history.extend(recent);
                last_layer = LAYER_AUTO_COMPACT.to_string();
            }
        }
    }

    (current_history, last_layer)
}

// ---------------------------------------------------------------------------
// Cache-aware compression
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub cache_zone_size: usize,
    pub compressible_zone_size: usize,
    pub messages_modified: usize,
    pub cache_zone_preserved: bool,
    pub compression_strategy: String,
    pub estimated_cache_hit: String,
    pub tokens_saved: usize,
}

impl Default for CacheStats {
    fn default() -> Self {
        CacheStats {
            cache_zone_size: 0,
            compressible_zone_size: 0,
            messages_modified: 0,
            cache_zone_preserved: true,
            compression_strategy: "none".to_string(),
            estimated_cache_hit: CACHE_HIT_STABLE.to_string(),
            tokens_saved: 0,
        }
    }
}

impl CacheStats {
    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::json!({
            "cache_zone_size": self.cache_zone_size,
            "compressible_zone_size": self.compressible_zone_size,
            "messages_modified": self.messages_modified,
            "cache_zone_preserved": self.cache_zone_preserved,
            "compression_strategy": self.compression_strategy,
            "estimated_cache_hit": self.estimated_cache_hit,
            "tokens_saved": self.tokens_saved,
        })
    }
}

pub struct CacheAwareCompressor {
    pub cache_zone_ratio: f64,
    pub min_cache_zone_messages: usize,
    pub batch_threshold: f64,
}

impl CacheAwareCompressor {
    pub fn new(
        cache_zone_ratio: Option<f64>,
        min_cache_zone_messages: Option<usize>,
        batch_threshold: Option<f64>,
    ) -> Self {
        CacheAwareCompressor {
            cache_zone_ratio: cache_zone_ratio.unwrap_or(DEFAULT_CACHE_ZONE_RATIO),
            min_cache_zone_messages: min_cache_zone_messages.unwrap_or(MIN_CACHE_ZONE_MESSAGES),
            batch_threshold: batch_threshold.unwrap_or(BATCH_COMPRESSION_THRESHOLD),
        }
    }

    pub fn compress(
        &self,
        history: &[Message],
        max_tokens: usize,
        usage_ratio: Option<f64>,
    ) -> (Vec<Message>, CacheStats) {
        if history.is_empty() {
            return (history.to_vec(), CacheStats::default());
        }
        let usage_ratio = match usage_ratio {
            Some(r) => r,
            None => calculate_usage_ratio(history, max_tokens),
        };

        if usage_ratio < self.batch_threshold {
            return (
                history.to_vec(),
                CacheStats {
                    cache_zone_size: history.len(),
                    compressible_zone_size: 0,
                    messages_modified: 0,
                    compression_strategy: "batch_threshold_not_reached".to_string(),
                    estimated_cache_hit: CACHE_HIT_STABLE.to_string(),
                    ..CacheStats::default()
                },
            );
        }

        let cache_zone_end = self.identify_cache_zone(history);
        let initial_tokens: usize = history.iter().map(|m| m.token_count()).sum();
        let (compressed, mut stats) = self.apply_cache_aware_compression(history, cache_zone_end, max_tokens);
        let final_tokens: usize = compressed.iter().map(|m| m.token_count()).sum();
        stats.tokens_saved = initial_tokens.saturating_sub(final_tokens);
        (compressed, stats)
    }

    fn identify_cache_zone(&self, history: &[Message]) -> usize {
        if history.is_empty() {
            return 0;
        }
        let ratio_based_size = ((history.len() as f64) * self.cache_zone_ratio) as usize;
        let mut cache_zone_size = ratio_based_size.max(self.min_cache_zone_messages);
        cache_zone_size = cache_zone_size.min(history.len());
        if history.len() - cache_zone_size < 2 {
            cache_zone_size = history.len().saturating_sub(2);
        }
        cache_zone_size
    }

    fn apply_cache_aware_compression(
        &self,
        history: &[Message],
        cache_zone_end: usize,
        max_tokens: usize,
    ) -> (Vec<Message>, CacheStats) {
        if cache_zone_end >= history.len() {
            return (
                history.to_vec(),
                CacheStats {
                    cache_zone_size: history.len(),
                    compressible_zone_size: 0,
                    messages_modified: 0,
                    cache_zone_preserved: true,
                    compression_strategy: "cache_zone_only".to_string(),
                    estimated_cache_hit: CACHE_HIT_STABLE.to_string(),
                    ..CacheStats::default()
                },
            );
        }

        let cache_zone = &history[..cache_zone_end];
        let compressible_zone = &history[cache_zone_end..];

        // Strategy 1: Microcompact compressible zone only
        let mut compressed_zone = microcompact(compressible_zone, 3);
        let mut messages_modified = compressed_zone.iter().filter(|m| m.compressed).count();

        let new_tokens: usize = compressed_zone.iter().map(|m| m.token_count()).sum();
        let cache_tokens: usize = cache_zone.iter().map(|m| m.token_count()).sum();
        let mut usage_ratio = (cache_tokens + new_tokens) as f64 / max_tokens as f64;

        // Strategy 2: If still over limit, apply budget_reduction
        if usage_ratio >= compression_threshold(LAYER_AUTO_COMPACT) {
            compressed_zone = budget_reduction(&compressed_zone);
            messages_modified += compressed_zone.iter().filter(|m| m.compressed).count();
        }

        // Strategy 3: If still over limit, apply snip (keep at least 2)
        let new_tokens: usize = compressed_zone.iter().map(|m| m.token_count()).sum();
        let cache_tokens: usize = cache_zone.iter().map(|m| m.token_count()).sum();
        usage_ratio = (cache_tokens + new_tokens) as f64 / max_tokens as f64;

        if usage_ratio >= compression_threshold(LAYER_AUTO_COMPACT) && compressed_zone.len() > 2 {
            messages_modified += compressed_zone.len() - 3;
            let take = compressed_zone.len().min(3);
            let keep = compressed_zone.len() - take;
            compressed_zone = compressed_zone[keep..].to_vec();
        }

        let estimated_hit = if messages_modified == 0 {
            CACHE_HIT_STABLE.to_string()
        } else if (messages_modified as f64) < (compressible_zone.len() as f64) * 0.5 {
            CACHE_HIT_PARTIAL.to_string()
        } else {
            CACHE_HIT_INVALIDATED.to_string()
        };

        let stats = CacheStats {
            cache_zone_size: cache_zone.len(),
            compressible_zone_size: compressible_zone.len(),
            messages_modified,
            cache_zone_preserved: true,
            compression_strategy: "cache_aware".to_string(),
            estimated_cache_hit: estimated_hit,
            ..CacheStats::default()
        };

        let mut result: Vec<Message> = cache_zone.to_vec();
        result.extend(compressed_zone);
        (result, stats)
    }
}

pub fn cache_aware_compress(
    history: &[Message],
    max_tokens: usize,
    cache_zone_ratio: Option<f64>,
) -> (Vec<Message>, CacheStats) {
    let compressor = CacheAwareCompressor::new(cache_zone_ratio, None, None);
    compressor.compress(history, max_tokens, None)
}

// ---------------------------------------------------------------------------
// Reactive compaction
// ---------------------------------------------------------------------------

pub struct ReactiveCompactor {
    pub structural_threshold: f64,
    pub semantic_threshold: f64,
    pub preserve_recent: usize,
    structural_triggered: bool,
    semantic_triggered: bool,
}

impl ReactiveCompactor {
    pub fn new(
        structural_threshold: Option<f64>,
        semantic_threshold: Option<f64>,
        preserve_recent: Option<usize>,
    ) -> Self {
        ReactiveCompactor {
            structural_threshold: structural_threshold.unwrap_or(STRUCTURAL_THRESHOLD),
            semantic_threshold: semantic_threshold.unwrap_or(SEMANTIC_THRESHOLD),
            preserve_recent: preserve_recent.unwrap_or(PRESERVE_RECENT),
            structural_triggered: false,
            semantic_triggered: false,
        }
    }

    pub fn reset_turn(&mut self) {
        self.structural_triggered = false;
        self.semantic_triggered = false;
    }

    /// Check usage and compact if threshold exceeded.
    /// Returns `(messages, layer_name)` — layer is None if no compaction.
    pub fn check_and_compact(
        &mut self,
        messages: &[serde_json::Value],
        max_tokens: usize,
    ) -> (Vec<serde_json::Value>, Option<String>) {
        if messages.is_empty() || max_tokens == 0 {
            return (messages.to_vec(), None);
        }

        let usage_ratio = self.calculate_usage_ratio(messages, max_tokens);

        // Semantic threshold first (higher priority). Python requires
        // `llm_client is not None` — the base port has no LLM client, so
        // this path is never armed (parity: falls through to structural).
        let has_llm_client = false;
        if usage_ratio >= self.semantic_threshold
            && !self.semantic_triggered
            && has_llm_client
            && messages.len() > self.preserve_recent + 2
        {
            self.semantic_triggered = true;
            let compacted = self.structural_compact(messages);
            return (compacted, Some("reactive_semantic".to_string()));
        }

        if usage_ratio >= self.structural_threshold
            && !self.structural_triggered
            && messages.len() > self.preserve_recent + 2
        {
            self.structural_triggered = true;
            let compacted = self.structural_compact(messages);
            return (compacted, Some("reactive_structural".to_string()));
        }

        (messages.to_vec(), None)
    }

    fn calculate_usage_ratio(&self, messages: &[serde_json::Value], max_tokens: usize) -> f64 {
        if messages.is_empty() {
            return 0.0;
        }
        let effective_max = if max_tokens > 0 {
            max_tokens
        } else {
            crate::token::DEFAULT_CONTEXT_WINDOW
        };
        let mut total_tokens: usize = 0;
        for msg in messages {
            if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                total_tokens += crate::token::estimate_tokens_simple(content);
                total_tokens += 4;
            }
        }
        if effective_max > 0 {
            total_tokens as f64 / effective_max as f64
        } else {
            0.0
        }
    }

    fn structural_compact(&self, messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
        let system_msgs: Vec<&serde_json::Value> = messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("system"))
            .collect();
        let conv_msgs: Vec<&serde_json::Value> = messages
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) != Some("system"))
            .collect();

        if conv_msgs.len() <= self.preserve_recent + 2 {
            return messages.to_vec();
        }

        let old_msgs = &conv_msgs[..conv_msgs.len() - self.preserve_recent];
        let recent_msgs = &conv_msgs[conv_msgs.len() - self.preserve_recent..];

        let mut timeline_parts = vec![format!("[Reactive Compaction: {} turns archived as timeline]", old_msgs.len())];

        for i in (0..old_msgs.len()).step_by(STRUCTURAL_BLOCK_SIZE) {
            let end = (i + STRUCTURAL_BLOCK_SIZE).min(old_msgs.len());
            let block = &old_msgs[i..end];

            let mut block_parts = vec![format!("  [{i}-{end}]")];

            let mut file_refs: Vec<String> = Vec::new();
            for msg in block {
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                for r in extract_file_refs(content) {
                    if !file_refs.contains(&r) {
                        file_refs.push(r);
                    }
                }
            }
            if !file_refs.is_empty() {
                file_refs.sort();
                let mut top: Vec<&str> = file_refs.iter().map(|s| s.as_str()).take(3).collect();
                top.sort_unstable();
                block_parts.push(format!("Files: {}", top.join(", ")));
            }

            let mut tool_counts: Vec<(String, usize)> = Vec::new();
            for msg in block {
                if msg.get("role").and_then(|v| v.as_str()) == Some("assistant") {
                    if let Some(tcs) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                        for tc in tcs {
                            let name = tc
                                .get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            if let Some(e) = tool_counts.iter_mut().find(|(n, _)| *n == name) {
                                e.1 += 1;
                            } else {
                                tool_counts.push((name, 1));
                            }
                        }
                    }
                }
            }
            if !tool_counts.is_empty() {
                tool_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
                let mut top: Vec<String> = tool_counts.iter().take(3).map(|(n, c)| format!("{n}({c})")).collect();
                top.sort();
                block_parts.push(format!("Tools: {}", top.join(", ")));
            }

            let mut user_intents: Vec<String> = Vec::new();
            for msg in block {
                if msg.get("role").and_then(|v| v.as_str()) == Some("user") {
                    if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
                        let first_line: String = content.split('\n').next().unwrap_or("").chars().take(60).collect::<String>().trim().to_string();
                        if !first_line.is_empty() && !user_intents.contains(&first_line) {
                            user_intents.push(first_line);
                        }
                    }
                }
            }
            if !user_intents.is_empty() {
                block_parts.push(format!("Tasks: {}", user_intents.iter().take(2).cloned().collect::<Vec<_>>().join("; ")));
            }

            if block_parts.len() > 1 {
                timeline_parts.push(block_parts.join(" | "));
            }
        }

        // Global stats
        let mut stats_parts = vec!["[Global]".to_string()];
        let user_turns = old_msgs.iter().filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("user")).count();
        let assistant_turns = old_msgs.iter().filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("assistant")).count();
        stats_parts.push(format!("Turns: {user_turns}u/{assistant_turns}a"));

        let total_tool_calls: usize = old_msgs
            .iter()
            .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("assistant"))
            .map(|m| m.get("tool_calls").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0))
            .sum();
        if total_tool_calls > 0 {
            stats_parts.push(format!("Tool calls: {total_tool_calls}"));
        }

        let errors = old_msgs
            .iter()
            .filter(|m| {
                m.get("role").and_then(|v| v.as_str()) == Some("tool")
                    && m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_lowercase().contains("error")
            })
            .count();
        if errors > 0 {
            stats_parts.push(format!("Errors: {errors}"));
        }

        if stats_parts.len() > 1 {
            timeline_parts.push(stats_parts.join(" "));
        }

        timeline_parts.push(format!("[Preserved: last {} turns for continuity]", recent_msgs.len()));
        let summary_text = timeline_parts.join("\n");

        let mut result: Vec<serde_json::Value> = system_msgs.iter().map(|m| (*m).clone()).collect();
        result.push(serde_json::json!({ "role": "system", "content": summary_text }));
        result.extend(recent_msgs.iter().map(|m| (*m).clone()));
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Message;

    fn msg(role: &str, content: impl Into<String>) -> Message {
        Message::new(
            role,
            serde_json::Value::String(content.into()),
            Vec::new(),
            None,
            String::new(),
            None,
            50,
            false,
            false,
            None,
            String::new(),
            String::new(),
            Vec::new(),
        )
    }

    fn tool_msg(content: impl Into<String>, tool_call_id: impl Into<String>, pinned: bool) -> Message {
        Message::new(
            "tool",
            serde_json::Value::String(content.into()),
            Vec::new(),
            Some(tool_call_id.into()),
            String::new(),
            None,
            20,
            false,
            pinned,
            None,
            String::new(),
            String::new(),
            Vec::new(),
        )
    }

    #[test]
    fn thresholds_match_python() {
        assert_eq!(compression_threshold("budget_reduction"), 0.60);
        assert_eq!(compression_threshold("snip"), 0.70);
        assert_eq!(compression_threshold("microcompact"), 0.80);
        assert_eq!(compression_threshold("context_collapse"), 0.90);
        assert_eq!(compression_threshold("auto_compact"), 0.95);
    }

    #[test]
    fn budget_reduction_replaces_large_tool_output() {
        let big = "x".repeat(5000);
        let history = vec![
            msg("user", "hello"),
            tool_msg(&big, "t1", false),
            msg("assistant", "done"),
        ];
        let out = budget_reduction(&history);
        assert!(out[1].compressed);
        assert!(out[1].content.as_str().unwrap().starts_with("[Tool result cleared: t1, 5000 chars]"));
        // Pinned is preserved
        let pinned = tool_msg(&big, "t2", true);
        let history2 = vec![pinned];
        let out2 = budget_reduction(&history2);
        assert!(!out2[0].compressed);
    }

    #[test]
    fn snip_drops_stale_turns() {
        let mut history = vec![msg("system", "sys")];
        for i in 0..20 {
            history.push(msg("user", format!("q{i}")));
            history.push(msg("assistant", format!("a{i}")));
        }
        let out = snip(&history, SNIP_KEEP_RECENT);
        // system + 8 recent conversation turns
        assert!(out.len() < history.len());
        assert_eq!(out[0].role, "system");
        assert!(out.len() >= 1 + SNIP_KEEP_RECENT);
    }

    #[test]
    fn microcompact_clears_old_tool_results() {
        let mut history = Vec::new();
        for i in 0..10 {
            history.push(tool_msg(format!("result {i}"), &format!("t{i}"), false));
        }
        let out = microcompact(&history, MICROCOMPACT_KEEP_RECENT);
        let cleared = out.iter().filter(|m| m.compressed).count();
        assert_eq!(cleared, 5); // 10 - keep_recent 5
        // Last 5 preserved
        assert!(!out[9].compressed);
    }

    #[test]
    fn context_collapse_produces_timeline() {
        let mut history = vec![msg("system", "sys")];
        for i in 0..40 {
            history.push(msg("user", format!("question {i} about file.py")));
            history.push(msg("assistant", format!("answer {i}")));
        }
        let out = context_collapse(&history);
        assert!(out.len() < history.len());
        let summary = out.iter().find(|m| m.content.as_str().map(|s| s.contains("[Context Collapse:")).unwrap_or(false));
        assert!(summary.is_some(), "timeline summary must exist");
        assert!(out[0].role == "system");
    }

    #[test]
    fn graduated_compress_no_op_below_threshold() {
        let history = vec![msg("user", "hi"), msg("assistant", "yo")];
        let (out, layer) = graduated_compress(&history, 0.5, Some(131072));
        assert_eq!(layer, LAYER_NONE);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn graduated_compress_l1_at_high_usage() {
        let big = "y".repeat(6000);
        let history = vec![
            tool_msg(&big, "t1", false),
            msg("user", "short"),
            msg("assistant", "short reply"),
        ];
        // Force ratio high by tiny max_tokens
        let (out, layer) = graduated_compress(&history, 0.99, Some(300));
        assert_ne!(layer, LAYER_NONE);
        // Some compression must have happened
        assert!(out.iter().any(|m| m.compressed) || out.len() < history.len());
    }

    #[test]
    fn cache_aware_skips_below_batch_threshold() {
        let history = vec![msg("user", "hi"), msg("assistant", "yo")];
        let c = CacheAwareCompressor::new(None, None, None);
        let (out, stats) = c.compress(&history, 131072, Some(0.1));
        assert_eq!(out.len(), 2);
        assert_eq!(stats.compression_strategy, "batch_threshold_not_reached");
    }

    #[test]
    fn cache_aware_preserves_cache_zone() {
        let mut history = Vec::new();
        for i in 0..10 {
            history.push(tool_msg(&format!("r{i}"), &format!("t{i}"), false));
        }
        let c = CacheAwareCompressor::new(Some(0.3), None, None);
        let (out, stats) = c.compress(&history, 50, Some(0.99));
        assert!(stats.cache_zone_preserved);
        // First cache_zone messages unchanged (not compressed)
        let cache_zone_size = ((10.0 * 0.3) as usize).max(5).min(8);
        for m in out.iter().take(cache_zone_size) {
            assert!(!m.compressed);
        }
        assert_eq!(stats.compression_strategy, "cache_aware");
    }

    #[test]
    fn reactive_structural_compact() {
        let mut r = ReactiveCompactor::new(None, None, None);
        let mut messages = vec![serde_json::json!({"role": "system", "content": "sys"})];
        for i in 0..20 {
            messages.push(serde_json::json!({"role": "user", "content": format!("question {i}")}));
            messages.push(serde_json::json!({"role": "assistant", "content": format!("answer {i}")}));
        }
        let (out, layer) = r.check_and_compact(&messages, 100);
        assert_eq!(layer.as_deref(), Some("reactive_structural"));
        assert!(out.len() < messages.len());
        assert!(out.iter().any(|m| m.get("content").and_then(|v| v.as_str()).map(|s| s.contains("[Reactive Compaction:")).unwrap_or(false)));

        // Per-turn: second call returns None (already triggered)
        let (out2, layer2) = r.check_and_compact(&messages, 100);
        assert!(layer2.is_none());
        assert_eq!(out2.len(), messages.len());

        // reset_turn re-arms
        r.reset_turn();
        let (_out3, layer3) = r.check_and_compact(&messages, 100);
        assert!(layer3.is_some());
    }
}
