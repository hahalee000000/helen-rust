//! Tests for compression module — graduated compression pipeline.

use helen_runtime::compression::*;
use helen_runtime::transcript::Message;
use serde_json::json;

fn make_msg(role: &str, content: &str) -> Message {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Message::new(
        role,
        json!(content),
        vec![],
        None,
        format!("test-msg-{id}"),
        None,
        0,
        false,
        false,
        None,
        String::new(),
        String::new(),
        vec![],
    )
}

// ── Constants ───────────────────────────────────────────────────────────

#[test]
fn compression_threshold_known() {
    assert!((compression_threshold("budget_reduction") - 0.60).abs() < 1e-9);
    assert!((compression_threshold("snip") - 0.70).abs() < 1e-9);
    assert!((compression_threshold("microcompact") - 0.80).abs() < 1e-9);
    assert!((compression_threshold("context_collapse") - 0.90).abs() < 1e-9);
    assert!((compression_threshold("auto_compact") - 0.95).abs() < 1e-9);
}

#[test]
fn compression_threshold_unknown() {
    assert!((compression_threshold("nonexistent") - 1.0).abs() < 1e-9);
}

#[test]
fn layer_constants() {
    assert_eq!(LAYER_NONE, "none");
    assert_eq!(LAYER_BUDGET_REDUCTION, "budget_reduction");
    assert_eq!(LAYER_SNIP, "snip");
    assert_eq!(LAYER_MICROCOMPACT, "microcompact");
    assert_eq!(LAYER_CONTEXT_COLLAPSE, "context_collapse");
    assert_eq!(LAYER_AUTO_COMPACT, "auto_compact");
}

#[test]
fn numeric_constants() {
    assert_eq!(BUDGET_REDUCTION_MAX_CHARS, 4000);
    assert_eq!(SNIP_KEEP_RECENT, 8);
    assert_eq!(MICROCOMPACT_KEEP_RECENT, 5);
    assert_eq!(CONTEXT_COLLAPSE_THRESHOLD, 20);
    assert!((DEFAULT_CACHE_ZONE_RATIO - 0.30).abs() < 1e-9);
    assert_eq!(MIN_CACHE_ZONE_MESSAGES, 5);
    assert!((BATCH_COMPRESSION_THRESHOLD - 0.75).abs() < 1e-9);
    assert_eq!(CACHE_HIT_STABLE, "stable");
    assert_eq!(CACHE_HIT_PARTIAL, "partial");
    assert_eq!(CACHE_HIT_INVALIDATED, "invalidated");
    assert!((STRUCTURAL_THRESHOLD - 0.90).abs() < 1e-9);
    assert!((SEMANTIC_THRESHOLD - 0.95).abs() < 1e-9);
    assert_eq!(PRESERVE_RECENT, 4);
    assert_eq!(STRUCTURAL_BLOCK_SIZE, 10);
}

// ── calculate_usage_ratio ───────────────────────────────────────────────

#[test]
fn usage_ratio_empty_history() {
    assert!((calculate_usage_ratio(&[], 1000) - 0.0).abs() < 1e-9);
}

#[test]
fn usage_ratio_zero_max_tokens() {
    assert!((calculate_usage_ratio(&[], 0) - 0.0).abs() < 1e-9);
}

#[test]
fn usage_ratio_basic() {
    let msgs = vec![
        make_msg("user", "hello"),
        make_msg("assistant", "world"),
    ];
    let ratio = calculate_usage_ratio(&msgs, 100);
    assert!(ratio > 0.0);
}

// ── graduated_compress ──────────────────────────────────────────────────

#[test]
fn graduated_compress_empty() {
    let history = vec![];
    let (result, layer) = graduated_compress(&history, 0.5, None);
    assert!(result.is_empty());
    assert_eq!(layer, LAYER_NONE);
}

#[test]
fn graduated_compress_low_usage() {
    let history = vec![make_msg("user", "hello")];
    let (result, layer) = graduated_compress(&history, 0.1, Some(100000));
    assert_eq!(layer, LAYER_NONE);
    assert_eq!(result.len(), 1);
}

#[test]
fn graduated_compress_high_usage() {
    let history = vec![
        make_msg("user", &"a".repeat(100)),
        make_msg("assistant", &"b".repeat(100)),
        make_msg("user", &"c".repeat(100)),
    ];
    let (result, layer) = graduated_compress(&history, 0.99, Some(10));
    assert!(layer != LAYER_NONE || result.len() <= history.len());
}

// ── CacheAwareCompressor ────────────────────────────────────────────────

#[test]
fn cache_aware_compressor_new_defaults() {
    let comp = CacheAwareCompressor::new(None, None, None);
    assert!((comp.cache_zone_ratio - DEFAULT_CACHE_ZONE_RATIO).abs() < 1e-9);
    assert_eq!(comp.min_cache_zone_messages, MIN_CACHE_ZONE_MESSAGES);
    assert!((comp.batch_threshold - BATCH_COMPRESSION_THRESHOLD).abs() < 1e-9);
}

#[test]
fn cache_aware_compressor_new_custom() {
    let comp = CacheAwareCompressor::new(Some(0.5), Some(10), Some(0.8));
    assert!((comp.cache_zone_ratio - 0.5).abs() < 1e-9);
    assert_eq!(comp.min_cache_zone_messages, 10);
    assert!((comp.batch_threshold - 0.8).abs() < 1e-9);
}

#[test]
fn cache_aware_compress_empty() {
    let history = vec![];
    let (result, _stats) = cache_aware_compress(&history, 1000, None);
    assert!(result.is_empty());
}

#[test]
fn cache_aware_compress_basic() {
    let history = vec![
        make_msg("user", "hello"),
        make_msg("assistant", "world"),
    ];
    let (result, _stats) = cache_aware_compress(&history, 1000, None);
    assert!(!result.is_empty());
}

// ── CacheStats ──────────────────────────────────────────────────────────

#[test]
fn cache_stats_default() {
    let stats = CacheStats::default();
    assert_eq!(stats.cache_zone_size, 0);
    assert_eq!(stats.tokens_saved, 0);
    assert_eq!(stats.compression_strategy, "none");
    assert_eq!(stats.estimated_cache_hit, CACHE_HIT_STABLE);
}

#[test]
fn cache_stats_to_dict() {
    let stats = CacheStats::default();
    let dict = stats.to_dict();
    assert!(dict.get("cache_zone_size").is_some());
    assert!(dict.get("tokens_saved").is_some());
}

// ── ReactiveCompactor ───────────────────────────────────────────────────

#[test]
fn reactive_compactor_new_defaults() {
    let comp = ReactiveCompactor::new(None, None, None);
    assert!((comp.structural_threshold - STRUCTURAL_THRESHOLD).abs() < 1e-9);
    assert!((comp.semantic_threshold - SEMANTIC_THRESHOLD).abs() < 1e-9);
    assert_eq!(comp.preserve_recent, PRESERVE_RECENT);
}

#[test]
fn reactive_compactor_new_custom() {
    let comp = ReactiveCompactor::new(Some(0.8), Some(0.9), Some(6));
    assert!((comp.structural_threshold - 0.8).abs() < 1e-9);
    assert!((comp.semantic_threshold - 0.9).abs() < 1e-9);
    assert_eq!(comp.preserve_recent, 6);
}

#[test]
fn reactive_compactor_reset_turn() {
    let mut comp = ReactiveCompactor::new(None, None, None);
    comp.reset_turn();
}

// ── Edge cases ──────────────────────────────────────────────────────────

#[test]
fn usage_ratio_single_message() {
    let msgs = vec![make_msg("user", "hello world this is a test message")];
    let ratio = calculate_usage_ratio(&msgs, 100);
    assert!(ratio >= 0.0);
}

#[test]
fn graduated_compress_with_max_tokens() {
    let history = vec![
        make_msg("user", "hello"),
        make_msg("assistant", "world"),
    ];
    let (_result, _layer) = graduated_compress(&history, 0.5, Some(4096));
}
