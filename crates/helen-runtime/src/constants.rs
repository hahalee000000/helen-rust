//! Centralized constants (Task 8.7).
//!
//! Byte-faithful port of `helen/runtime/constants.py` (v1.45.0). All
//! hardcoded values (URLs, model names, thresholds, limits) are defined
//! here to ensure consistency. A parity test asserts every constant
//! matches the Python source (see `tests/constants_parity.rs`).

// ── LLM Configuration Defaults ─────────────────────────────────

pub const DEFAULT_MODEL: &str = "qwen3.7-plus";
pub const DEFAULT_BASE_URL: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1";
pub const DEFAULT_FALLBACK_URL: &str = "https://coding.dashscope.aliyuncs.com/v1";
pub const DEFAULT_TEMPERATURE: f64 = 0.7;
pub const DEFAULT_TIMEOUT: u64 = 60;
pub const DEFAULT_MAX_TURNS: usize = 10;

// ── Token Estimation ───────────────────────────────────────────

/// Crude heuristic: ~4 characters per token (English text).
pub const CHARS_PER_TOKEN: usize = 4;
pub const MAX_HISTORY_TOKENS: usize = 128_000;
pub const HISTORY_BUFFER_TOKENS: usize = 1_000;

// ── Fuzzy Match Thresholds ────────────────────────────────────

pub const FUZZY_EXACT_THRESHOLD: f64 = 1.0;
pub const FUZZY_HIGH_THRESHOLD: f64 = 0.80;
pub const FUZZY_MEDIUM_THRESHOLD: f64 = 0.70;
pub const FUZZY_LOW_THRESHOLD: f64 = 0.50;
pub const FUZZY_MIN_THRESHOLD: f64 = 0.3;

// ── Tool Limits ────────────────────────────────────────────────

/// Characters.
pub const MAX_READ_FILE_SIZE: usize = 16_000;
/// 64 MB.
pub const MAX_WRITE_FILE_SIZE: usize = 64 * 1024 * 1024;
/// Characters for tool output.
pub const MAX_OUTPUT_SIZE: usize = 8_000;
/// Characters for diff output.
pub const MAX_DIFF_SIZE: usize = 4_000;
/// 100 MB.
pub const MAX_DOWNLOAD_SIZE: usize = 100 * 1024 * 1024;
/// 8 MB.
pub const MAX_RESPONSE_SIZE: usize = 8 * 1024 * 1024;

// ── Timeout Defaults ──────────────────────────────────────────

/// Seconds.
pub const DEFAULT_TOOL_TIMEOUT: u64 = 30;
pub const DEFAULT_SHELL_TIMEOUT: u64 = 30;
pub const DEFAULT_FETCH_TIMEOUT: u64 = 15;
pub const DEFAULT_DOWNLOAD_TIMEOUT: u64 = 60;
pub const MAX_COMMAND_TIMEOUT: u64 = 300;

// ── HTTP Configuration ────────────────────────────────────────

pub const DEFAULT_USER_AGENT: &str = "Helen/1.0";
pub const AGENT_USER_AGENT: &str = "HelenAgent/1.0 (https://github.com/hahalee000000/helen)";
/// Bytes for download chunks.
pub const DEFAULT_CHUNK_SIZE: usize = 8192;

// ── Wikipedia API ──────────────────────────────────────────────

pub const WIKI_SUMMARY_URL: &str = "https://en.wikipedia.org/api/rest_v1/page/summary/";
pub const WIKI_SEARCH_URL: &str = "https://en.wikipedia.org/w/api.php";

// ── Config Paths ───────────────────────────────────────────────

pub const CONFIG_FILENAME: &str = "config.yaml";
pub const HELEN_HOME_DIRNAME: &str = ".helen";
