//! Token estimation utilities (Task 5.5) — port of `helen/runtime/token_utils.py`.
//!
//! Character-type-aware heuristic (~15% accuracy without tiktoken).

pub const CHARS_PER_TOKEN_EN: f64 = 4.0;
pub const CHARS_PER_TOKEN_CJK: f64 = 1.2;
pub const CHARS_PER_TOKEN_MIXED: f64 = 3.0;
pub const DEFAULT_CONTEXT_WINDOW: usize = 131072;

/// Check if a character is CJK (Chinese/Japanese/Korean) — comprehensive
/// Unicode range check mirroring `token_utils.is_cjk`.
pub fn is_cjk_char(c: char) -> bool {
    let cp = c as u32;
    (0x4E00..=0x9FFF).contains(&cp) // CJK Unified Ideographs
        || (0x3400..=0x4DBF).contains(&cp) // CJK Extension A
        || (0x20000..=0x2A6DF).contains(&cp) // CJK Extension B
        || (0x2A700..=0x2B73F).contains(&cp) // CJK Extension C
        || (0x2B740..=0x2B81F).contains(&cp) // CJK Extension D
        || (0xF900..=0xFAFF).contains(&cp) // CJK Compatibility Ideographs
        || (0x3000..=0x303F).contains(&cp) // CJK Symbols and Punctuation
        || (0x3040..=0x309F).contains(&cp) // Hiragana
        || (0x30A0..=0x30FF).contains(&cp) // Katakana
        || (0xAC00..=0xD7AF).contains(&cp) // Hangul Syllables
}

/// Estimate token count using character-type-aware heuristic.
/// Returns 0 for empty text; >= 1 otherwise (Python parity).
pub fn estimate_tokens_simple(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let cjk_count = text.chars().filter(|c| is_cjk_char(*c)).count();
    let total_len = text.chars().count();

    if cjk_count == 0 {
        // Pure Latin/English
        ((total_len as f64 / CHARS_PER_TOKEN_EN) as usize).max(1)
    } else if cjk_count == total_len {
        // Pure CJK
        ((total_len as f64 / CHARS_PER_TOKEN_CJK) as usize).max(1)
    } else {
        // Mixed content
        let non_cjk = total_len - cjk_count;
        ((cjk_count as f64 / CHARS_PER_TOKEN_CJK + non_cjk as f64 / CHARS_PER_TOKEN_EN) as usize)
            .max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_cjk() {
        assert!(is_cjk_char('中'));
        assert!(is_cjk_char('文'));
        assert!(is_cjk_char('あ'));
        assert!(is_cjk_char('한'));
        assert!(!is_cjk_char('a'));
        assert!(!is_cjk_char('1'));
        assert!(!is_cjk_char(' '));
    }

    #[test]
    fn test_estimate_empty() {
        assert_eq!(estimate_tokens_simple(""), 0);
    }

    #[test]
    fn test_estimate_english() {
        // "hello world" = 11 chars / 4 = 2
        assert_eq!(estimate_tokens_simple("hello world"), 2);
        // At least 1 for non-empty
        assert_eq!(estimate_tokens_simple("a"), 1);
    }

    #[test]
    fn test_estimate_cjk() {
        // 4 CJK chars / 1.2 = 3
        assert_eq!(estimate_tokens_simple("中文测试"), 3);
    }

    #[test]
    fn test_estimate_mixed() {
        // "hello世界" — 5 latin + 2 cjk = 2/4 + 2/1.2 = 0 + 1 = 1 (int div)
        let n = estimate_tokens_simple("hello世界");
        assert!(n >= 1);
    }
}
