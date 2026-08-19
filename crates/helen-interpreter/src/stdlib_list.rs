//! std.list — List manipulation stdlib functions.
//!
//! Ports Python's list operations: sort, map, filter, reduce, unique, etc.

use std::cell::RefCell;
use std::rc::Rc;

use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;
use crate::stdlib::StdlibExport;
use crate::stdlib_helpers::{arg_int, arg_list, err_expected};

fn list_sort(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
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
    let mut items = items;
    // Optional compare closure: Python `sorted(lst, key=cmp_to_key(compare))`.
    if let Some(cmp_fn) = args.get(1) {
        items.sort_by(|a, b| {
            let r = interp.call_value(cmp_fn.clone(), vec![a.clone(), b.clone()]);
            match r {
                Ok(Value::Int(n)) => n.to_i64().unwrap_or(0).cmp(&0),
                Ok(v) => v.truthy().cmp(&true),
                Err(_) => std::cmp::Ordering::Equal,
            }
        });
    } else {
        items.sort_by(|a, b| {
            crate::interpreter_builtins::cmp_values(a, b).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn list_map(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
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
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => out.push(v),
            Err(e) => {
                let item_repr = item.python_repr();
                let truncated = if item_repr.len() > 100 {
                    format!("{}...(truncated)", &item_repr[..100])
                } else {
                    item_repr
                };
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!(
                        "map operation failed at index {i}: {} (element: {truncated})",
                        e.message
                    ),
                    e.span,
                ));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_filter(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
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
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if v.truthy() {
                    out.push(item.clone());
                }
            }
            Err(e) => {
                let item_repr = item.python_repr();
                let truncated = if item_repr.len() > 100 {
                    format!("{}...(truncated)", &item_repr[..100])
                } else {
                    item_repr
                };
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!(
                        "filter operation failed at index {i}: {} (element: {truncated})",
                        e.message
                    ),
                    e.span,
                ));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_reduce(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
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
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    let initial = args.get(2).cloned().unwrap_or(Value::Null);
    let initial_is_null = matches!(initial, Value::Null);
    let mut acc = if initial_is_null {
        match items.first() {
            Some(first) => first.clone(),
            None => {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    "reduce() of empty sequence with no initial value".to_string(),
                    None,
                ))
            }
        }
    } else {
        initial
    };
    let start = if initial_is_null { 1 } else { 0 };
    for item in items.iter().skip(start) {
        match interp.call_value(f.clone(), vec![acc, item.clone()]) {
            Ok(v) => acc = v,
            Err(e) => {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!("reduce operation failed: {}", e.message),
                    e.span,
                ))
            }
        }
    }
    Ok(acc)
}

fn list_unique(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = match args.first() {
        Some(Value::List(l)) => l.borrow().clone(),
        Some(v) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("expected list, got {}", v.type_name()),
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
    let mut out: Vec<Value> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_flatten(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let mut out = Vec::new();
    for item in items {
        match item {
            Value::List(l) => {
                for sub in l.borrow().iter() {
                    out.push(sub.clone());
                }
            }
            other => out.push(other),
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_chunk(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let size = arg_int(args, 1)?.to_i64().unwrap_or(0).max(0) as usize;
    if size == 0 {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: chunk size must be positive".to_string(),
            None,
        ));
    }
    let mut out = Vec::new();
    for chunk in items.chunks(size) {
        out.push(Value::List(Rc::new(RefCell::new(chunk.to_vec()))));
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_zip(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let lists: Vec<Vec<Value>> = args
        .iter()
        .map(|a| match a {
            Value::List(l) => Ok(l.borrow().clone()),
            v => err_expected("list", v),
        })
        .collect::<Result<_, _>>()?;
    if lists.is_empty() {
        return Ok(Value::List(Rc::new(RefCell::new(vec![]))));
    }
    let min_len = lists.iter().map(|l| l.len()).min().unwrap_or(0);
    let mut out = Vec::new();
    for i in 0..min_len {
        let row: Vec<Value> = lists.iter().map(|l| l[i].clone()).collect();
        // Python zip() → list of tuples
        out.push(Value::Tuple(Rc::new(RefCell::new(row))));
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn list_every(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if !v.truthy() {
                    return Ok(Value::Bool(false));
                }
            }
            Err(e) => {
                return Err(wrap_hof_error("every", i, item, e));
            }
        }
    }
    Ok(Value::Bool(true))
}

fn list_some(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if v.truthy() {
                    return Ok(Value::Bool(true));
                }
            }
            Err(e) => {
                return Err(wrap_hof_error("some", i, item, e));
            }
        }
    }
    Ok(Value::Bool(false))
}

fn list_find_if(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let items = arg_list(args, 0)?;
    let f = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing function".to_string(), None))?;
    for (i, item) in items.iter().enumerate() {
        match interp.call_value(f.clone(), vec![item.clone()]) {
            Ok(v) => {
                if v.truthy() {
                    return Ok(item.clone());
                }
            }
            Err(e) => {
                return Err(wrap_hof_error("find", i, item, e));
            }
        }
    }
    Ok(Value::Null)
}

fn wrap_hof_error(op: &str, index: usize, item: &Value, e: ExceptionValue) -> ExceptionValue {
    let item_repr = item.python_repr();
    let truncated = if item_repr.len() > 100 {
        format!("{}...(truncated)", &item_repr[..100])
    } else {
        item_repr
    };
    ExceptionValue::new(
        "RuntimeError",
        format!(
            "{op} operation failed at index {index}: {} (element: {truncated})",
            e.message
        ),
        e.span,
    )
}


pub static LIST_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "sort",
        func: list_sort,
    },
    StdlibExport {
        name: "map",
        func: list_map,
    },
    StdlibExport {
        name: "filter",
        func: list_filter,
    },
    StdlibExport {
        name: "reduce",
        func: list_reduce,
    },
    StdlibExport {
        name: "unique",
        func: list_unique,
    },
    StdlibExport {
        name: "flatten",
        func: list_flatten,
    },
    StdlibExport {
        name: "chunk",
        func: list_chunk,
    },
    StdlibExport {
        name: "zip",
        func: list_zip,
    },
    StdlibExport {
        name: "every",
        func: list_every,
    },
    StdlibExport {
        name: "some",
        func: list_some,
    },
    StdlibExport {
        name: "find_if",
        func: list_find_if,
    },
];
