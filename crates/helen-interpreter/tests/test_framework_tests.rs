//! Tests for test_framework module — TestRegistry, TestSuite, TestReport.

use helen_interpreter::test_framework::*;

// ── TestRegistry tests ──────────────────────────────────────────────────

#[test]
fn registry_new() {
    let reg = TestRegistry::new();
    assert!(reg.suites.is_empty());
    assert!(reg.current_suite.is_none());
    assert!(reg.results.is_empty());
    assert!(!reg.running);
    assert!(reg.warnings.is_empty());
    assert_eq!(reg.test_timeout, 30.0);
}

#[test]
fn registry_reset() {
    let mut reg = TestRegistry::new();
    reg.suites.push(TestSuite {
        name: "test".into(),
        tests: vec![],
        before_each: None,
        after_each: None,
        before_all: None,
        after_all: None,
    });
    reg.running = true;
    reg.reset();
    assert!(reg.suites.is_empty());
    assert!(!reg.running);
}

#[test]
fn registry_start_suite() {
    let mut reg = TestRegistry::new();
    reg.start_suite("my_suite");
    assert_eq!(reg.suites.len(), 1);
    assert_eq!(reg.suites[0].name, "my_suite");
    assert_eq!(reg.current_suite, Some(0));
}

#[test]
fn registry_end_suite() {
    let mut reg = TestRegistry::new();
    reg.start_suite("suite1");
    reg.end_suite();
    assert!(reg.current_suite.is_none());
}

#[test]
fn registry_register_test() {
    let mut reg = TestRegistry::new();
    reg.start_suite("suite");
    reg.register_test("test1", None, false);
    assert_eq!(reg.suites[0].tests.len(), 1);
    assert_eq!(reg.suites[0].tests[0].name, "test1");
    assert!(!reg.suites[0].tests[0].skip);
}

#[test]
fn registry_register_test_skip() {
    let mut reg = TestRegistry::new();
    reg.start_suite("suite");
    reg.register_test("skipped_test", None, true);
    assert!(reg.suites[0].tests[0].skip);
}

#[test]
fn registry_set_timeout() {
    let mut reg = TestRegistry::new();
    reg.set_timeout(60.0);
    assert_eq!(reg.test_timeout, 60.0);
}

// ── TestSuite tests ─────────────────────────────────────────────────────

#[test]
fn test_suite_new() {
    let suite = TestSuite {
        name: "my_suite".into(),
        tests: vec![],
        before_each: None,
        after_each: None,
        before_all: None,
        after_all: None,
    };
    assert_eq!(suite.name, "my_suite");
    assert!(suite.tests.is_empty());
    assert!(suite.before_each.is_none());
    assert!(suite.after_each.is_none());
}

// ── TestCase tests ──────────────────────────────────────────────────────

#[test]
fn test_case_new() {
    let case = TestCase {
        name: "test1".into(),
        body: None,
        skip: false,
    };
    assert_eq!(case.name, "test1");
    assert!(case.body.is_none());
    assert!(!case.skip);
}

#[test]
fn test_case_skip() {
    let case = TestCase {
        name: "skipped".into(),
        body: None,
        skip: true,
    };
    assert!(case.skip);
}

// ── TestResult tests ────────────────────────────────────────────────────

#[test]
fn test_result_passed() {
    let result = TestResult {
        name: "test1".into(),
        suite: "suite1".into(),
        passed: true,
        error: None,
        duration_ms: 10.5,
    };
    assert!(result.passed);
    assert!(result.error.is_none());
    assert_eq!(result.duration_ms, 10.5);
}

#[test]
fn test_result_failed() {
    let result = TestResult {
        name: "test2".into(),
        suite: "suite1".into(),
        passed: false,
        error: Some("assertion failed".into()),
        duration_ms: 5.0,
    };
    assert!(!result.passed);
    assert_eq!(result.error.as_deref(), Some("assertion failed"));
}

// ── TestReport tests ────────────────────────────────────────────────────

#[test]
fn test_report_new() {
    let report = TestReport {
        suites: vec![],
        results: vec![],
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        duration_ms: 0.0,
        warnings: vec![],
    };
    assert_eq!(report.total, 0);
    assert_eq!(report.passed, 0);
    assert_eq!(report.failed, 0);
    assert_eq!(report.skipped, 0);
    assert!(report.warnings.is_empty());
}

#[test]
fn test_report_with_results() {
    let report = TestReport {
        suites: vec![],
        results: vec![
            TestResult {
                name: "t1".into(),
                suite: "s1".into(),
                passed: true,
                error: None,
                duration_ms: 1.0,
            },
            TestResult {
                name: "t2".into(),
                suite: "s1".into(),
                passed: false,
                error: Some("err".into()),
                duration_ms: 2.0,
            },
        ],
        total: 2,
        passed: 1,
        failed: 1,
        skipped: 0,
        duration_ms: 3.0,
        warnings: vec![],
    };
    assert_eq!(report.total, 2);
    assert_eq!(report.passed, 1);
    assert_eq!(report.failed, 1);
}

// ── Clone tests ─────────────────────────────────────────────────────────

#[test]
fn test_suite_clone() {
    let suite = TestSuite {
        name: "s".into(),
        tests: vec![TestCase {
            name: "t".into(),
            body: None,
            skip: false,
        }],
        before_each: None,
        after_each: None,
        before_all: None,
        after_all: None,
    };
    let cloned = suite.clone();
    assert_eq!(cloned.tests.len(), 1);
}

#[test]
fn test_result_clone() {
    let result = TestResult {
        name: "t".into(),
        suite: "s".into(),
        passed: true,
        error: None,
        duration_ms: 1.0,
    };
    let cloned = result.clone();
    assert!(cloned.passed);
}

// ── Debug tests ─────────────────────────────────────────────────────────

#[test]
fn test_registry_debug() {
    let reg = TestRegistry::new();
    let debug = format!("{:?}", reg);
    assert!(debug.contains("TestRegistry"));
}

#[test]
fn test_suite_debug() {
    let suite = TestSuite {
        name: "s".into(),
        tests: vec![],
        before_each: None,
        after_each: None,
        before_all: None,
        after_all: None,
    };
    let debug = format!("{:?}", suite);
    assert!(debug.contains("TestSuite"));
}

#[test]
fn test_result_debug() {
    let result = TestResult {
        name: "t".into(),
        suite: "s".into(),
        passed: true,
        error: None,
        duration_ms: 1.0,
    };
    let debug = format!("{:?}", result);
    assert!(debug.contains("TestResult"));
}
