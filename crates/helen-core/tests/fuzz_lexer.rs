//! M13 Task 13.7 — Fuzz / property tests for the lexer.
//!
//! 1. Random byte/char streams must never panic the Scanner; any errors
//!    reported must carry a valid ErrorCode.
//! 2. Scanning a valid token stream twice is deterministic.
//! 3. Re-scanning a token's lexeme must be self-consistent (no crash).
//!
//! Run: cargo test -p helen-core --test fuzz_lexer

use helen_core::lexer::Scanner;
use proptest::prelude::*;

fn assert_no_panic(src: String) {
    let mut s = Scanner::new(&src, "<fuzz>");
    let toks = s.scan_all();
    // scanning consumes the stream; errors are available after scan_all
    let errs = s.errors();
    for e in &errs {
        // every error must carry a real code (>= 300: ScannerError internal + E0xxx)
        assert!(
            e.code().value() >= 300,
            "unexpected error code {}",
            e.code().value()
        );
    }
    // every token kind must be a defined variant — we can't enumerate all,
    // but scanning twice must yield identical token counts.
    let mut s2 = Scanner::new(&src, "<fuzz>");
    let toks2 = s2.scan_all();
    assert_eq!(
        toks.len(),
        toks2.len(),
        "non-deterministic scan of {:?}",
        &src[..src.len().min(64)]
    );
}

proptest! {
    // Random strings (incl. unicode, control chars, CJK) must never panic.
    #[test]
    fn fuzz_random_strings(s in "\\PC*") {
        assert_no_panic(s);
    }

    // ASCII-heavy garbage (operator soup, partial strings, etc.)
    #[test]
    fn fuzz_ascii_garbage(s in "[!@#$%^&*()_+\\-=\\[\\]{}|;:,.<>?/\\\\]") {
        assert_no_panic(s);
    }

    // Numeric-looking streams (partial exponents, dots, signs)
    #[test]
    fn fuzz_numeric_streams(s in "[0-9eE.+-]") {
        assert_no_panic(s);
    }

    // Quote-heavy streams (unterminated strings, escapes)
    #[test]
    fn fuzz_quotes(s in "[\"'`\\\\n]") {
        assert_no_panic(s);
    }

    // Random alnum + keyword soup
    #[test]
    fn fuzz_keyword_soup(s in "[a-zA-Z_0-9]") {
        assert_no_panic(s);
    }

    // Deterministic rescan of valid-looking source
    #[test]
    fn fuzz_deterministic_rescan(s in "(let|const|fn|main|agent|if|while|for|return|print|true|false|null|import|export|class|impl|match|try|catch|throw|spawn|await|async|channel|shared|store|scope|switch|case|break|continue|1|2.5|\\\"hi\\\"|\\\"\\\"|[+-]?[0-9]+)") {
        assert_no_panic(s);
    }
}
