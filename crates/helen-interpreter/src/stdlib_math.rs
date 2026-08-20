//! std.math — Mathematical operations and statistics.
//!
//! Ports Python's math module: pow, sqrt, log, trig, statistics, etc.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::stdlib::StdlibExport;
use crate::stdlib_helpers::{arg_bool, arg_f64, arg_int, arg_list, err_expected};
use crate::value::Value;

// std.math
// ---------------------------------------------------------------------------

fn math_pow(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let base = match args.first() {
        Some(Value::Int(n)) => n.to_f64().unwrap_or(0.0),
        Some(Value::Float(f)) => *f,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected number, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let exp = match args.get(1) {
        Some(Value::Int(n)) => n.to_f64().unwrap_or(0.0),
        Some(Value::Float(f)) => *f,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected number, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    let result = base.powf(exp);
    // Parity with Python `math.pow`: raising a finite base to a finite
    // exponent whose result overflows raises OverflowError (surfaced as
    // RuntimeError here); `inf`/`nan` inputs pass through unchecked.
    if base.is_finite() && exp.is_finite() && !result.is_finite() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python OverflowError: math range error".to_string(),
            None,
        ));
    }
    Ok(Value::Float(result))
}

/// Python `round(value, ndigits)` — banker's rounding (round-half-even),
/// int input stays int, float input stays float.
fn math_round(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument".to_string(), None))?;
    let ndigits = match args.get(1) {
        Some(Value::Int(n)) => Some(n.clone()),
        Some(Value::Null) | None => None,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected int for ndigits, got {}", v.type_name()),
                None,
            ))
        }
    };
    match value {
        Value::Int(n) => {
            // Python: int round with ndigits=0 returns int unchanged;
            // negative ndigits rounds to the nearest power of 10.
            let is_zero = ndigits
                .as_ref()
                .map(|d| d.sign() == num_bigint::Sign::NoSign)
                .unwrap_or(true);
            if is_zero {
                Ok(Value::Int(n))
            } else if let Some(d) = &ndigits {
                if d.sign() == num_bigint::Sign::Minus {
                    // round-half-even to nearest 10^|d|
                    let p = d.to_i64().unwrap_or(0).unsigned_abs() as u32;
                    let pow = BigInt::from(10).pow(p);
                    let q = &n / &pow;
                    let r = &n % &pow;
                    let half = &pow / BigInt::from(2);
                    let rounded = if r > half {
                        q + BigInt::from(1)
                    } else if r == half {
                        // half-even: round to even quotient
                        if (&q % BigInt::from(2)).sign() == num_bigint::Sign::NoSign {
                            q
                        } else {
                            q + BigInt::from(1)
                        }
                    } else {
                        q
                    };
                    Ok(Value::Int(rounded * pow))
                } else {
                    Ok(Value::Int(n))
                }
            } else {
                Ok(Value::Int(n))
            }
        }
        Value::Float(f) => {
            let digits = match ndigits {
                None => 0i32,
                Some(d) => d.to_i32().unwrap_or(0),
            };
            if digits == 0 {
                Ok(Value::Float(f.round_ties_even()))
            } else {
                let factor = 10f64.powi(digits);
                Ok(Value::Float((f * factor).round_ties_even() / factor))
            }
        }
        v => Err(ExceptionValue::new(
            "RuntimeError",
            format!("round() argument must be a number, got {}", v.type_name()),
            None,
        )),
    }
}

fn math_sqrt(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = match args.first() {
        Some(Value::Int(n)) => n.to_f64().unwrap_or(0.0),
        Some(Value::Float(f)) => *f,
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected number, got {}", v.type_name()),
                None,
            ))
        }
        None => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                "missing argument".to_string(),
                None,
            ))
        }
    };
    if x < 0.0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: math domain error".to_string(),
            None,
        ));
    }
    Ok(Value::Float(x.sqrt()))
}

fn math_floor(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    match args.first() {
        Some(Value::Float(f)) => Ok(Value::Int(BigInt::from(f.floor() as i64))),
        Some(Value::Int(n)) => Ok(Value::Int(n.clone())),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected number, got {}", v.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "missing argument".to_string(),
            None,
        )),
    }
}

fn math_ceil(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    match args.first() {
        Some(Value::Float(f)) => Ok(Value::Int(BigInt::from(f.ceil() as i64))),
        Some(Value::Int(n)) => Ok(Value::Int(n.clone())),
        Some(v) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected number, got {}", v.type_name()),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            "missing argument".to_string(),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// std.math — statistics (mean, median, mode, variance, stddev, ...)
// ---------------------------------------------------------------------------

fn math_list_f64(args: &[Value], i: usize) -> Result<Vec<f64>, ExceptionValue> {
    let items = arg_list(args, i)?;
    let mut out = Vec::with_capacity(items.len());
    for v in items {
        match v {
            Value::Int(n) => out.push(n.to_f64().ok_or_else(|| {
                ExceptionValue::new("RuntimeError", "number out of range".to_string(), None)
            })?),
            Value::Float(f) => out.push(f),
            other => return err_expected("number", &other),
        }
    }
    Ok(out)
}

fn math_empty_err(op: &str) -> ExceptionValue {
    ExceptionValue::new(
        "RuntimeError",
        format!("Python ValueError: Cannot calculate {op} of empty list"),
        None,
    )
}

fn math_mean(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    if nums.is_empty() {
        return Err(math_empty_err("mean"));
    }
    Ok(Value::Float(nums.iter().sum::<f64>() / nums.len() as f64))
}

fn math_median(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    // Python _median: odd length returns the middle element unchanged (ints
    // stay ints); even length is float division.
    let mut items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("median"));
    }
    // Sort by numeric value
    items.sort_by(|a, b| {
        let fa = match a {
            Value::Int(n) => n.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        let fb = match b {
            Value::Int(n) => n.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        fa.partial_cmp(&fb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let n = items.len();
    let mid = n / 2;
    if n % 2 == 0 {
        let a = match &items[mid - 1] {
            Value::Int(x) => x.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        let b = match &items[mid] {
            Value::Int(x) => x.to_f64().unwrap_or(0.0),
            Value::Float(f) => *f,
            _ => 0.0,
        };
        Ok(Value::Float((a + b) / 2.0))
    } else {
        Ok(items[mid].clone())
    }
}

fn math_mode(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("mode"));
    }
    // Python _mode: Counter on ORIGINAL values — ints stay ints (no float coercion)
    use std::collections::HashMap;
    let mut counts: HashMap<Vec<u8>, usize> = HashMap::new();
    for v in &items {
        *counts.entry(mode_key(v)).or_insert(0) += 1;
    }
    let max_count = counts.values().cloned().max().unwrap_or(0);
    let mut modes: Vec<Value> = Vec::new();
    for v in &items {
        if counts.get(&mode_key(v)) == Some(&max_count) && !modes.contains(v) {
            modes.push(v.clone());
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(modes))))
}

/// Numeric `a < b` across int/float (Python comparison semantics).
fn num_lt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x < y,
        _ => {
            let fa = num_as_f64(a);
            let fb = num_as_f64(b);
            fa < fb
        }
    }
}

/// Numeric `a > b` across int/float (Python comparison semantics).
fn num_gt(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x > y,
        _ => {
            let fa = num_as_f64(a);
            let fb = num_as_f64(b);
            fa > fb
        }
    }
}

/// Extract a numeric value as f64 for cross-type comparisons.
fn num_as_f64(v: &Value) -> f64 {
    match v {
        Value::Int(n) => n.to_f64().unwrap_or(0.0),
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

/// Canonical key for mode counting: bytes + type tag so `1` and `1.0` stay distinct.
fn mode_key(v: &Value) -> Vec<u8> {
    match v {
        Value::Int(n) => {
            let mut k = vec![0u8];
            k.extend_from_slice(&n.to_signed_bytes_be());
            k
        }
        Value::Float(f) => {
            let mut k = vec![1u8];
            k.extend_from_slice(&f.to_bits().to_le_bytes());
            k
        }
        Value::Str(s) => {
            let mut k = vec![2u8];
            k.extend_from_slice(s.as_bytes());
            k
        }
        Value::Bool(b) => vec![3u8, *b as u8],
        other => {
            let mut k = vec![4u8];
            k.extend_from_slice(format!("{other:?}").as_bytes());
            k
        }
    }
}

fn math_variance(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    // Python: population=True is the DEFAULT (only sample when 2nd arg present)
    let population = if args.len() > 1 {
        arg_bool(args, 1)?
    } else {
        true
    };
    if nums.is_empty() {
        return Err(math_empty_err("variance"));
    }
    if !population && nums.len() < 2 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Sample variance requires at least 2 values".to_string(),
            None,
        ));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let sq: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum();
    let denom = if population {
        nums.len() as f64
    } else {
        (nums.len() - 1) as f64
    };
    Ok(Value::Float(sq / denom))
}

fn math_stddev(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    // Python: population=True is the DEFAULT (only sample when 2nd arg present)
    let population = if args.len() > 1 {
        arg_bool(args, 1)?
    } else {
        true
    };
    if nums.is_empty() {
        return Err(math_empty_err("standard deviation"));
    }
    if !population && nums.len() < 2 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Sample variance requires at least 2 values".to_string(),
            None,
        ));
    }
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    let sq: f64 = nums.iter().map(|x| (x - mean).powi(2)).sum();
    let denom = if population {
        nums.len() as f64
    } else {
        (nums.len() - 1) as f64
    };
    Ok(Value::Float((sq / denom).sqrt()))
}

fn math_correlation(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = math_list_f64(args, 0)?;
    let y = math_list_f64(args, 1)?;
    if x.is_empty() || y.is_empty() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Cannot calculate correlation of empty lists".to_string(),
            None,
        ));
    }
    if x.len() != y.len() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Lists must have the same length".to_string(),
            None,
        ));
    }
    let n = x.len() as f64;
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let cov: f64 = x
        .iter()
        .zip(y.iter())
        .map(|(xi, yi)| (xi - mx) * (yi - my))
        .sum::<f64>()
        / n;
    let sx = (x.iter().map(|v| (v - mx).powi(2)).sum::<f64>() / n).sqrt();
    let sy = (y.iter().map(|v| (v - my).powi(2)).sum::<f64>() / n).sqrt();
    if sx == 0.0 || sy == 0.0 {
        return Ok(Value::Float(0.0));
    }
    Ok(Value::Float(cov / (sx * sy)))
}

fn math_percentile(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut nums = math_list_f64(args, 0)?;
    let p = arg_f64(args, 1)?;
    if nums.is_empty() {
        return Err(math_empty_err("percentile"));
    }
    if !(0.0..=100.0).contains(&p) {
        return Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Percentile must be between 0 and 100, got {p}"),
            None,
        ));
    }
    nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = nums.len();
    if p == 0.0 {
        return Ok(Value::Float(nums[0]));
    }
    if p == 100.0 {
        return Ok(Value::Float(nums[n - 1]));
    }
    let k = (n - 1) as f64 * (p / 100.0);
    let f = k.floor();
    let c = k.ceil();
    if f == c {
        Ok(Value::Float(nums[k as usize]))
    } else {
        Ok(Value::Float(
            nums[f as usize] * (c - k) + nums[c as usize] * (k - f),
        ))
    }
}

fn math_sum(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    // Python sum(): preserves ints when all inputs are ints
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Ok(Value::Int(BigInt::from(0)));
    }
    let mut all_int = true;
    for v in &items {
        if !matches!(v, Value::Int(_)) {
            all_int = false;
            break;
        }
    }
    if all_int {
        let mut total = BigInt::from(0);
        for v in &items {
            if let Value::Int(n) = v {
                total += n;
            }
        }
        Ok(Value::Int(total))
    } else {
        let mut total = 0.0f64;
        for v in &items {
            let f = match v {
                Value::Int(n) => n.to_f64().ok_or_else(|| {
                    ExceptionValue::new("RuntimeError", "number out of range".to_string(), None)
                })?,
                Value::Float(f) => *f,
                other => return err_expected("number", other),
            };
            total += f;
        }
        Ok(Value::Float(total))
    }
}

fn math_product(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let nums = math_list_f64(args, 0)?;
    if nums.is_empty() {
        return Ok(Value::Float(1.0));
    }
    Ok(Value::Float(nums.iter().product()))
}

fn math_stats_min(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("minimum"));
    }
    // Python _min: preserves ints for int inputs
    let mut best = items[0].clone();
    for v in &items[1..] {
        if num_lt(v, &best) {
            best = v.clone();
        }
    }
    Ok(best)
}

fn math_stats_max(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    if items.is_empty() {
        return Err(math_empty_err("maximum"));
    }
    // Python _max: preserves ints for int inputs
    let mut best = items[0].clone();
    for v in &items[1..] {
        if num_gt(v, &best) {
            best = v.clone();
        }
    }
    Ok(best)
}

// ── Trig ───────────────────────────────────────────────────────

macro_rules! math_unary {
    ($name:ident, $f:expr) => {
        fn $name(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
            let x = arg_f64(args, 0)?;
            Ok(Value::Float($f(x)))
        }
    };
}

math_unary!(math_cos, f64::cos);
math_unary!(math_sin, f64::sin);
math_unary!(math_tan, f64::tan);
math_unary!(math_acos, f64::acos);
math_unary!(math_asin, f64::asin);
math_unary!(math_atan, f64::atan);
math_unary!(math_exp, f64::exp);
math_unary!(math_log10, f64::log10);

fn math_atan2(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let y = arg_f64(args, 0)?;
    let x = arg_f64(args, 1)?;
    Ok(Value::Float(y.atan2(x)))
}

fn math_log(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = arg_f64(args, 0)?;
    if x <= 0.0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Logarithm requires positive number".to_string(),
            None,
        ));
    }
    match args.get(1) {
        Some(Value::Null) | None => Ok(Value::Float(x.ln())),
        Some(_) => {
            let base = arg_f64(args, 1)?;
            Ok(Value::Float(x.log(base)))
        }
    }
}

fn math_log2(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let x = arg_f64(args, 0)?;
    if x <= 0.0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Logarithm requires positive number".to_string(),
            None,
        ));
    }
    Ok(Value::Float(x.log2()))
}

// ── Bitwise (v1.39.4) ──────────────────────────────────────────

fn math_bit_and(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let b = arg_int(args, 1)?;
    Ok(Value::Int(a & b))
}

fn math_bit_or(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let b = arg_int(args, 1)?;
    Ok(Value::Int(a | b))
}

fn math_bit_xor(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let b = arg_int(args, 1)?;
    Ok(Value::Int(a ^ b))
}

fn math_bit_not(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    Ok(Value::Int(-a - 1))
}

fn math_bit_shift_left(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let n = arg_int(args, 1)?;
    let n_u = n.to_u32().ok_or_else(|| {
        ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Shift amount must be non-negative".to_string(),
            None,
        )
    })?;
    if n.sign() == num_bigint::Sign::Minus {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Shift amount must be non-negative".to_string(),
            None,
        ));
    }
    Ok(Value::Int(a << n_u))
}

fn math_bit_shift_right(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let a = arg_int(args, 0)?;
    let n = arg_int(args, 1)?;
    if n.sign() == num_bigint::Sign::Minus {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: Shift amount must be non-negative".to_string(),
            None,
        ));
    }
    let n_u = n.to_u32().unwrap_or(0);
    Ok(Value::Int(a >> n_u))
}

// ---------------------------------------------------------------------------
pub static MATH_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "pow",
        func: math_pow,
    },
    StdlibExport {
        name: "round",
        func: math_round,
    },
    StdlibExport {
        name: "sqrt",
        func: math_sqrt,
    },
    StdlibExport {
        name: "floor",
        func: math_floor,
    },
    StdlibExport {
        name: "ceil",
        func: math_ceil,
    },
    StdlibExport {
        name: "mean",
        func: math_mean,
    },
    StdlibExport {
        name: "median",
        func: math_median,
    },
    StdlibExport {
        name: "mode",
        func: math_mode,
    },
    StdlibExport {
        name: "variance",
        func: math_variance,
    },
    StdlibExport {
        name: "stddev",
        func: math_stddev,
    },
    StdlibExport {
        name: "correlation",
        func: math_correlation,
    },
    StdlibExport {
        name: "percentile",
        func: math_percentile,
    },
    StdlibExport {
        name: "sum",
        func: math_sum,
    },
    StdlibExport {
        name: "product",
        func: math_product,
    },
    StdlibExport {
        name: "stats_min",
        func: math_stats_min,
    },
    StdlibExport {
        name: "stats_max",
        func: math_stats_max,
    },
    StdlibExport {
        name: "cos",
        func: math_cos,
    },
    StdlibExport {
        name: "sin",
        func: math_sin,
    },
    StdlibExport {
        name: "tan",
        func: math_tan,
    },
    StdlibExport {
        name: "acos",
        func: math_acos,
    },
    StdlibExport {
        name: "asin",
        func: math_asin,
    },
    StdlibExport {
        name: "atan",
        func: math_atan,
    },
    StdlibExport {
        name: "atan2",
        func: math_atan2,
    },
    StdlibExport {
        name: "log",
        func: math_log,
    },
    StdlibExport {
        name: "log2",
        func: math_log2,
    },
    StdlibExport {
        name: "log10",
        func: math_log10,
    },
    StdlibExport {
        name: "exp",
        func: math_exp,
    },
    StdlibExport {
        name: "bit_and",
        func: math_bit_and,
    },
    StdlibExport {
        name: "bit_or",
        func: math_bit_or,
    },
    StdlibExport {
        name: "bit_xor",
        func: math_bit_xor,
    },
    StdlibExport {
        name: "bit_not",
        func: math_bit_not,
    },
    StdlibExport {
        name: "bit_shift_left",
        func: math_bit_shift_left,
    },
    StdlibExport {
        name: "bit_shift_right",
        func: math_bit_shift_right,
    },
];
