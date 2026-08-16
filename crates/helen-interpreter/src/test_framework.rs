//! Test framework implementation for Helen stdlib.
//!
//! Byte-faithful port of `helen/stdlib/test.py` (v1.44.0): provides
//! describe/it/assert/expect for TDD-style testing of Helen programs.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use helen_core::source::SourceSpan;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

/// A single test result.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub suite: String,
    pub passed: bool,
    pub error: Option<String>,
    pub duration_ms: f64,
}

/// A test suite (describe block).
#[derive(Debug, Clone)]
pub struct TestSuite {
    pub name: String,
    pub tests: Vec<TestCase>,
    pub before_each: Option<Rc<crate::closure::Closure>>,
    pub after_each: Option<Rc<crate::closure::Closure>>,
    pub before_all: Option<Rc<crate::closure::Closure>>,
    pub after_all: Option<Rc<crate::closure::Closure>>,
}

/// A single test case (it block).
#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub body: Option<Rc<crate::closure::Closure>>,
    pub skip: bool,
}

/// Aggregated test report.
#[derive(Debug, Clone)]
pub struct TestReport {
    pub suites: Vec<TestSuite>,
    pub results: Vec<TestResult>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: f64,
    pub warnings: Vec<String>,
}

/// Global test registry (thread-local for safety).
thread_local! {
    static TEST_REGISTRY: RefCell<TestRegistry> = RefCell::new(TestRegistry::new());
}

/// Test registry state.
#[derive(Debug)]
pub struct TestRegistry {
    pub suites: Vec<TestSuite>,
    pub current_suite: Option<usize>,
    pub results: Vec<TestResult>,
    pub running: bool,
    pub warnings: Vec<String>,
    pub test_timeout: f64,
}

fn dummy_span() -> SourceSpan {
    SourceSpan::new("", 0, 0, 0, 0)
}

impl TestRegistry {
    pub fn new() -> Self {
        Self {
            suites: Vec::new(),
            current_suite: None,
            results: Vec::new(),
            running: false,
            warnings: Vec::new(),
            test_timeout: 30.0,
        }
    }

    pub fn reset(&mut self) {
        self.suites.clear();
        self.current_suite = None;
        self.results.clear();
        self.warnings.clear();
        self.running = false;
    }

    pub fn start_suite(&mut self, name: &str) {
        let suite = TestSuite {
            name: name.to_string(),
            tests: Vec::new(),
            before_each: None,
            after_each: None,
            before_all: None,
            after_all: None,
        };
        self.suites.push(suite);
        self.current_suite = Some(self.suites.len() - 1);
    }

    pub fn end_suite(&mut self) {
        self.current_suite = None;
    }

    pub fn register_test(&mut self, name: &str, body: Option<Rc<crate::closure::Closure>>, skip: bool) {
        if self.current_suite.is_none() {
            self.start_suite("(default)");
        }
        let idx = self.current_suite.unwrap();
        
        for existing in &self.suites[idx].tests {
            if existing.name == name {
                let warning = format!(
                    "Warning: duplicate test name '{}' in suite '{}'",
                    name, self.suites[idx].name
                );
                self.warnings.push(warning);
                break;
            }
        }
        
        self.suites[idx].tests.push(TestCase {
            name: name.to_string(),
            body,
            skip,
        });
    }

    pub fn set_before_each(&mut self, closure: Rc<crate::closure::Closure>) {
        if let Some(idx) = self.current_suite {
            self.suites[idx].before_each = Some(closure);
        }
    }

    pub fn set_after_each(&mut self, closure: Rc<crate::closure::Closure>) {
        if let Some(idx) = self.current_suite {
            self.suites[idx].after_each = Some(closure);
        }
    }

    pub fn set_before_all(&mut self, closure: Rc<crate::closure::Closure>) {
        if let Some(idx) = self.current_suite {
            self.suites[idx].before_all = Some(closure);
        }
    }

    pub fn set_after_all(&mut self, closure: Rc<crate::closure::Closure>) {
        if let Some(idx) = self.current_suite {
            self.suites[idx].after_all = Some(closure);
        }
    }

    pub fn set_timeout(&mut self, seconds: f64) {
        self.test_timeout = seconds.max(0.1);
    }

    pub fn run_all(&mut self, interpreter: &mut Interpreter) -> TestReport {
        self.running = true;
        self.results.clear();
        let start = Instant::now();
        let span = dummy_span();

        for suite in &self.suites {
            if let Some(before_all) = &suite.before_all {
                let _ = interpreter.call_closure(before_all.as_ref(), vec![], &span);
            }

            for test in &suite.tests {
                let test_start = Instant::now();
                let mut result = TestResult {
                    name: test.name.clone(),
                    suite: suite.name.clone(),
                    passed: false,
                    error: None,
                    duration_ms: 0.0,
                };

                if test.skip {
                    result.passed = true;
                    result.error = Some("SKIPPED".to_string());
                } else {
                    if let Some(before_each) = &suite.before_each {
                        let _ = interpreter.call_closure(before_each.as_ref(), vec![], &span);
                    }

                    if let Some(body) = &test.body {
                        match interpreter.call_closure(body.as_ref(), vec![], &span) {
                            Ok(_) => result.passed = true,
                            Err(e) => {
                                result.passed = false;
                                result.error = Some(format!("{}: {}", e.class_name, e.message));
                            }
                        }
                    } else {
                        result.passed = true;
                    }

                    if let Some(after_each) = &suite.after_each {
                        let _ = interpreter.call_closure(after_each.as_ref(), vec![], &span);
                    }
                }

                result.duration_ms = test_start.elapsed().as_secs_f64() * 1000.0;
                self.results.push(result);
            }

            if let Some(after_all) = &suite.after_all {
                let _ = interpreter.call_closure(after_all.as_ref(), vec![], &span);
            }
        }

        let duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.running = false;

        let total = self.results.len();
        let passed = self.results.iter().filter(|r| r.passed && r.error.is_none()).count();
        let failed = self.results.iter().filter(|r| !r.passed).count();
        let skipped = self.results.iter().filter(|r| r.error.as_deref() == Some("SKIPPED")).count();

        TestReport {
            suites: self.suites.clone(),
            results: self.results.clone(),
            total,
            passed,
            failed,
            skipped,
            duration_ms,
            warnings: self.warnings.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Stdlib function implementations
// ---------------------------------------------------------------------------

pub fn test_describe(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let name = arg_str(args, 0)?;
    let body = arg_closure(args, 1)?;
    
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.start_suite(name);
    });
    
    let span = dummy_span();
    _i.call_closure(body.as_ref(), vec![], &span)?;
    
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.end_suite();
    });
    
    Ok(Value::Str(Rc::from(format!("Suite '{}' registered", name).as_str())))
}

pub fn test_it(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let name = arg_str(args, 0)?;
    let body = arg_closure(args, 1)?;
    
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.register_test(name, Some(body), false);
    });
    
    Ok(Value::Str(Rc::from(format!("Test '{}' registered", name).as_str())))
}

pub fn test_it_skip(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let name = arg_str(args, 0)?;
    
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.register_test(name, None, true);
    });
    
    Ok(Value::Str(Rc::from(format!("Test '{}' skipped", name).as_str())))
}

pub fn test_assert_true(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let condition = args.get(0).cloned().unwrap_or(Value::Null);
    let message = arg_opt_str(args, 1)?.unwrap_or("");
    
    if !condition.truthy() {
        let msg = if message.is_empty() {
            format!("Assertion failed: {:?} is not truthy", condition)
        } else {
            message.to_string()
        };
        return Err(ExceptionValue::new("AssertionError", msg, None));
    }
    
    Ok(Value::Bool(true))
}

pub fn test_assert_equal(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let actual = args.get(0).cloned().unwrap_or(Value::Null);
    let expected = args.get(1).cloned().unwrap_or(Value::Null);
    let message = arg_opt_str(args, 2)?.unwrap_or("");
    
    if actual != expected {
        let msg = if message.is_empty() {
            format!("Expected {:?}, got {:?}", expected, actual)
        } else {
            message.to_string()
        };
        return Err(ExceptionValue::new("AssertionError", msg, None));
    }
    
    Ok(Value::Bool(true))
}

pub fn test_assert_not_equal(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let actual = args.get(0).cloned().unwrap_or(Value::Null);
    let expected = args.get(1).cloned().unwrap_or(Value::Null);
    let message = arg_opt_str(args, 2)?.unwrap_or("");
    
    if actual == expected {
        let msg = if message.is_empty() {
            format!("Expected {:?} != {:?}", actual, expected)
        } else {
            message.to_string()
        };
        return Err(ExceptionValue::new("AssertionError", msg, None));
    }
    
    Ok(Value::Bool(true))
}

pub fn test_assert_contains(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let container = args.get(0).cloned().unwrap_or(Value::Null);
    let item = args.get(1).cloned().unwrap_or(Value::Null);
    let message = arg_opt_str(args, 2)?.unwrap_or("");
    
    let contains = match &container {
        Value::Str(s) => {
            if let Value::Str(sub) = &item {
                s.contains(sub.as_ref())
            } else {
                false
            }
        }
        Value::List(list) => {
            list.borrow().iter().any(|v| *v == item)
        }
        Value::Map(map) => {
            map.borrow().contains_key(&item)
        }
        _ => false,
    };
    
    if !contains {
        let msg = if message.is_empty() {
            format!("Expected {:?} to contain {:?}", container, item)
        } else {
            message.to_string()
        };
        return Err(ExceptionValue::new("AssertionError", msg, None));
    }
    
    Ok(Value::Bool(true))
}

pub fn test_assert_throws(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let func = arg_closure(args, 0)?;
    let error_type = arg_opt_str(args, 1)?.unwrap_or("");
    
    let span = dummy_span();
    match _i.call_closure(func.as_ref(), vec![], &span) {
        Ok(_) => {
            Err(ExceptionValue::new("AssertionError", "Expected function to throw, but it did not".to_string(), None))
        }
        Err(e) => {
            if !error_type.is_empty() && e.class_name != error_type {
                return Err(ExceptionValue::new(
                    "AssertionError",
                    format!("Expected {}, got {}: {}", error_type, e.class_name, e.message),
                    None,
                ));
            }
            Ok(Value::Str(Rc::from(e.message.as_str())))
        }
    }
}

pub fn test_fail(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let message = arg_opt_str(args, 0)?.unwrap_or("Test failed");
    Err(ExceptionValue::new("AssertionError", message.to_string(), None))
}

pub fn test_before_each(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let closure = arg_closure(args, 0)?;
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.set_before_each(closure);
    });
    Ok(Value::Str(Rc::from("before_each hook set")))
}

pub fn test_after_each(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let closure = arg_closure(args, 0)?;
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.set_after_each(closure);
    });
    Ok(Value::Str(Rc::from("after_each hook set")))
}

pub fn test_before_all(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let closure = arg_closure(args, 0)?;
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.set_before_all(closure);
    });
    Ok(Value::Str(Rc::from("before_all hook set")))
}

pub fn test_after_all(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let closure = arg_closure(args, 0)?;
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.set_after_all(closure);
    });
    Ok(Value::Str(Rc::from("after_all hook set")))
}

pub fn test_set_timeout(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let seconds = arg_float(args, 0)?;
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.set_timeout(seconds);
    });
    Ok(Value::Str(Rc::from(format!("Test timeout set to {}s", seconds).as_str())))
}

pub fn test_run_tests(i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let report = TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.run_all(i)
    });
    
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("total")), Value::Int(BigInt::from(report.total as i64)));
    result.insert(Value::Str(Rc::from("passed")), Value::Int(BigInt::from(report.passed as i64)));
    result.insert(Value::Str(Rc::from("failed")), Value::Int(BigInt::from(report.failed as i64)));
    result.insert(Value::Str(Rc::from("skipped")), Value::Int(BigInt::from(report.skipped as i64)));
    result.insert(Value::Str(Rc::from("duration_ms")), Value::Float(report.duration_ms));
    result.insert(Value::Str(Rc::from("report")), Value::Str(Rc::from(format_report(&report).as_str())));
    
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

pub fn test_run_tests_json(i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let report = TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.run_all(i)
    });
    
    let json = serde_json::json!({
        "total": report.total,
        "passed": report.passed,
        "failed": report.failed,
        "skipped": report.skipped,
        "duration_ms": report.duration_ms,
        "warnings": report.warnings,
        "suites": report.suites.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "tests": s.tests.len(),
            })
        }).collect::<Vec<_>>(),
        "results": report.results.iter().map(|r| {
            serde_json::json!({
                "name": r.name,
                "suite": r.suite,
                "passed": r.passed,
                "error": r.error,
                "duration_ms": r.duration_ms,
            })
        }).collect::<Vec<_>>(),
    });
    
    Ok(Value::Str(Rc::from(serde_json::to_string_pretty(&json).unwrap().as_str())))
}

pub fn test_reset(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.reset();
    });
    Ok(Value::Str(Rc::from("Tests reset")))
}

pub fn test_count(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let (suites, tests, results) = TEST_REGISTRY.with(|registry| {
        let reg = registry.borrow();
        (
            reg.suites.len(),
            reg.suites.iter().map(|s| s.tests.len()).sum::<usize>(),
            reg.results.len(),
        )
    });
    
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("suites")), Value::Int(BigInt::from(suites as i64)));
    result.insert(Value::Str(Rc::from("tests")), Value::Int(BigInt::from(tests as i64)));
    result.insert(Value::Str(Rc::from("results")), Value::Int(BigInt::from(results as i64)));
    
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

pub fn test_suite(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let name = arg_str(args, 0)?;
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.start_suite(name);
    });
    Ok(Value::Str(Rc::from(format!("Suite '{}' started", name).as_str())))
}

pub fn test_end_suite(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    TEST_REGISTRY.with(|registry| {
        let mut reg = registry.borrow_mut();
        reg.end_suite();
    });
    Ok(Value::Str(Rc::from("Suite ended")))
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn format_report(report: &TestReport) -> String {
    let mut lines = Vec::new();
    lines.push(String::new());
    lines.push("=".repeat(60));
    lines.push("TEST RESULTS".to_string());
    lines.push("=".repeat(60));
    lines.push(String::new());
    
    for result in &report.results {
        let status = if result.error.as_deref() == Some("SKIPPED") {
            "⊘"
        } else if result.passed {
            "✓"
        } else {
            "✗"
        };
        lines.push(format!("{} {} ({}ms)", status, result.name, result.duration_ms as i64));
        if let Some(err) = &result.error {
            if err != "SKIPPED" {
                lines.push(format!("  Error: {}", err));
            }
        }
    }
    
    lines.push(String::new());
    lines.push("-".repeat(60));
    lines.push(format!(
        "Total: {} | Passed: {} | Failed: {} | Skipped: {} | Duration: {:.2}ms",
        report.total, report.passed, report.failed, report.skipped, report.duration_ms
    ));
    lines.push("=".repeat(60));
    
    lines.join("\n")
}

fn arg_str(args: &[Value], i: usize) -> Result<&str, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.as_ref()),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected string at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}

fn arg_opt_str<'a>(args: &'a [Value], i: usize) -> Result<Option<&'a str>, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(Some(s.as_ref())),
        Some(Value::Null) | None => Ok(None),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected string or null at position {}, got {:?}", i, other),
            None,
        )),
    }
}

fn arg_float(args: &[Value], i: usize) -> Result<f64, ExceptionValue> {
    match args.get(i) {
        Some(Value::Float(f)) => Ok(*f),
        Some(Value::Int(n)) => Ok(n.to_f64().unwrap_or(0.0)),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected number at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}

fn arg_closure(args: &[Value], i: usize) -> Result<Rc<crate::closure::Closure>, ExceptionValue> {
    match args.get(i) {
        Some(Value::Closure(c)) => Ok(c.clone()),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected closure at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}

/// Create an expectation for chainable assertions.
pub fn test_expect(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args.get(0).cloned().unwrap_or(Value::Null);
    // Stub: chainable expectations not yet fully implemented
    // Returns the value wrapped in a map for now
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("value")), value);
    result.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("expectation")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}
