//! Extended tests for compression module — constants and simple functions.

use helen_runtime::compression::*;

// ── Constants tests ─────────────────────────────────────────────────────

#[test]
fn compression_threshold_known() {
    assert_eq!(compression_threshold("budget_reduction"), 0.60);
    assert_eq!(compression_threshold("snip"), 0.70);
    assert_eq!(compression_threshold("microcompact"), 0.80);
    assert_eq!(compression_threshold("context_collapse"), 0.90);
    assert_eq!(compression_threshold("auto_compact"), 0.95);
}

#[test]
fn compression_threshold_unknown() {
    assert_eq!(compression_threshold("nonexistent"), 1.0);
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
fn budget_reduction_constants() {
    assert_eq!(BUDGET_REDUCTION_MAX_CHARS, 4000);
    assert_eq!(SNIP_KEEP_RECENT, 8);
    assert_eq!(MICROCOMPACT_KEEP_RECENT, 5);
    assert_eq!(CONTEXT_COLLAPSE_THRESHOLD, 20);
}

#[test]
fn cache_aware_constants() {
    assert_eq!(DEFAULT_CACHE_ZONE_RATIO, 0.30);
    assert_eq!(MIN_CACHE_ZONE_MESSAGES, 5);
    assert_eq!(BATCH_COMPRESSION_THRESHOLD, 0.75);
    assert_eq!(CACHE_HIT_STABLE, "stable");
    assert_eq!(CACHE_HIT_PARTIAL, "partial");
    assert_eq!(CACHE_HIT_INVALIDATED, "invalidated");
}

#[test]
fn reactive_compaction_constants() {
    assert_eq!(STRUCTURAL_THRESHOLD, 0.90);
    assert_eq!(SEMANTIC_THRESHOLD, 0.95);
    assert_eq!(PRESERVE_RECENT, 4);
    assert_eq!(STRUCTURAL_BLOCK_SIZE, 10);
}
