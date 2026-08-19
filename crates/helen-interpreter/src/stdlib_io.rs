//! std.io — File system and I/O operations.
//!
//! Ports Python's os/path/io modules: file ops, directory ops, path manipulation.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;
use crate::stdlib::StdlibExport;
use crate::stdlib_helpers::{arg_str, arg_f64, arg_bool, arg_opt_str, arg_opt_int};

// std.path
// ---------------------------------------------------------------------------

fn path_join(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let parts: Vec<String> = args.iter().map(|v| v.python_str()).collect();
    let mut p = std::path::PathBuf::new();
    for part in &parts {
        p.push(part);
    }
    Ok(Value::Str(Rc::from(
        p.to_string_lossy().to_string().as_str(),
    )))
}

fn path_basename(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let p = std::path::Path::new(s);
    Ok(Value::Str(Rc::from(
        p.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
            .as_str(),
    )))
}

fn path_dirname(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    let p = std::path::Path::new(s);
    Ok(Value::Str(Rc::from(
        p.parent()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default()
            .as_str(),
    )))
}

fn path_exists(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Bool(std::path::Path::new(s).exists()))
}

fn path_is_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Bool(std::path::Path::new(s).is_file()))
}

fn path_is_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let s = arg_str(args, 0)?;
    Ok(Value::Bool(std::path::Path::new(s).is_dir()))
}

// ---------------------------------------------------------------------------
// std.io
// ---------------------------------------------------------------------------

fn io_write_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let content = arg_str(args, 1)?;
    std::fs::write(path, content)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Wrote {path}").as_str())))
}

fn io_append_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let content = arg_str(args, 1)?;
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    f.write_all(content.as_bytes())
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Appended to {path}").as_str())))
}

fn io_mkdir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    std::fs::create_dir(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Created directory {path}").as_str(),
    )))
}

fn io_mkdir_p(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    std::fs::create_dir_all(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Created directory {path}").as_str(),
    )))
}

fn io_stream_print(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    _i.stdout.lock().expect("mutex poisoned").push_str(text);
    _i.stdout.lock().expect("mutex poisoned").push('\n');
    Ok(Value::Str(Rc::from(text)))
}

fn io_stream_clear(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn io_stream_cursor_up(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn io_stream_cursor_down(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn io_progress_bar(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let current = arg_f64(args, 0)?;
    let total = arg_f64(args, 1)?;
    let width = arg_opt_int(args, 2)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(40)
        .max(1) as usize;
    let pct = if total > 0.0 {
        (current / total * 100.0).min(100.0)
    } else {
        0.0
    };
    let filled = ((pct / 100.0) * width as f64) as usize;
    let bar = format!(
        "[{}>{}] {:.0}%",
        "#".repeat(filled),
        "-".repeat(width.saturating_sub(filled + 1)),
        pct
    );
    Ok(Value::Str(Rc::from(bar.as_str())))
}

// ---------------------------------------------------------------------------
// std.file
// ---------------------------------------------------------------------------

fn file_file_size(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let meta = std::fs::metadata(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Int(BigInt::from(meta.len() as i64)))
}

fn file_file_modified(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let meta = std::fs::metadata(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    Ok(Value::Float(mtime))
}

fn file_list_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let entries = std::fs::read_dir(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    let items: Vec<Value> = entries
        .filter_map(|e| e.ok())
        .map(|e| {
            Value::Str(Rc::from(
                e.file_name().to_string_lossy().to_string().as_str(),
            ))
        })
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn file_walk_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let mut out: Vec<Value> = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<Value>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                let s = Value::Str(Rc::from(p.to_string_lossy().to_string().as_str()));
                if p.is_dir() {
                    out.push(s);
                    walk(&p, out);
                } else {
                    out.push(s);
                }
            }
        }
    }
    walk(std::path::Path::new(path), &mut out);
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn file_copy_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let src = arg_str(args, 0)?;
    let dst = arg_str(args, 1)?;
    std::fs::copy(src, dst)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Copied {src} to {dst}").as_str(),
    )))
}

fn file_move_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let src = arg_str(args, 0)?;
    let dst = arg_str(args, 1)?;
    std::fs::rename(src, dst)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(
        format!("Moved {src} to {dst}").as_str(),
    )))
}

fn file_delete_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    std::fs::remove_file(path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Deleted {path}").as_str())))
}

fn file_delete_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let recursive = arg_bool(args, 1).unwrap_or(false);
    if recursive {
        std::fs::remove_dir_all(path).map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None)
        })?;
    } else {
        std::fs::remove_dir(path).map_err(|e| {
            ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None)
        })?;
    }
    Ok(Value::Str(Rc::from(
        format!("Deleted directory {path}").as_str(),
    )))
}

fn file_temp_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let suffix = arg_opt_str(args, 0)?.unwrap_or("");
    let prefix = arg_opt_str(args, 1)?.unwrap_or("tmp");
    let default_dir = std::env::temp_dir().to_string_lossy().to_string();
    let dir = arg_opt_str(args, 2)?.unwrap_or(&default_dir);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = format!("{dir}/{prefix}{nanos}{suffix}");
    std::fs::write(&path, "")
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(path.as_str())))
}

fn file_temp_dir(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let suffix = arg_opt_str(args, 0)?.unwrap_or("");
    let prefix = arg_opt_str(args, 1)?.unwrap_or("tmp");
    let default_dir = std::env::temp_dir().to_string_lossy().to_string();
    let dir = arg_opt_str(args, 2)?.unwrap_or(&default_dir);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = format!("{dir}/{prefix}{nanos}{suffix}");
    std::fs::create_dir(&path)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(path.as_str())))
}

fn file_glob_files(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let pattern = arg_opt_str(args, 1)?.unwrap_or("*");
    let re = regex::Regex::new(&glob_to_regex(pattern)).unwrap();
    let mut out: Vec<Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if re.is_match(&name) {
                out.push(Value::Str(Rc::from(
                    e.path().to_string_lossy().to_string().as_str(),
                )));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn glob_to_regex(glob: &str) -> String {
    let mut re = String::from("^");
    for c in glob.chars() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' | '+' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                re.push('\\');
                re.push(c);
            }
            _ => re.push(c),
        }
    }
    re.push('$');
    re
}

fn file_grep_files(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let pattern = arg_str(args, 1)?;
    let re = regex::Regex::new(pattern).map_err(|e| {
        ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None)
    })?;
    let mut out: Vec<Value> = Vec::new();
    if let Ok(content) = std::fs::read_to_string(path) {
        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                out.push(Value::Str(Rc::from(format!("{}:{}", i + 1, line).as_str())));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

// ---------------------------------------------------------------------------
pub static PATH_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "path_join",
        func: path_join,
    },
    StdlibExport {
        name: "path_basename",
        func: path_basename,
    },
    StdlibExport {
        name: "path_dirname",
        func: path_dirname,
    },
    StdlibExport {
        name: "path_exists",
        func: path_exists,
    },
    StdlibExport {
        name: "path_is_file",
        func: path_is_file,
    },
    StdlibExport {
        name: "path_is_dir",
        func: path_is_dir,
    },
];

pub static IO_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "write_file",
        func: io_write_file,
    },
    StdlibExport {
        name: "append_file",
        func: io_append_file,
    },
    StdlibExport {
        name: "mkdir",
        func: io_mkdir,
    },
    StdlibExport {
        name: "mkdir_p",
        func: io_mkdir_p,
    },
    StdlibExport {
        name: "stream_print",
        func: io_stream_print,
    },
    StdlibExport {
        name: "stream_clear",
        func: io_stream_clear,
    },
    StdlibExport {
        name: "stream_cursor_up",
        func: io_stream_cursor_up,
    },
    StdlibExport {
        name: "stream_cursor_down",
        func: io_stream_cursor_down,
    },
    StdlibExport {
        name: "progress_bar",
        func: io_progress_bar,
    },
];

pub static FILE_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "file_size",
        func: file_file_size,
    },
    StdlibExport {
        name: "file_modified",
        func: file_file_modified,
    },
    StdlibExport {
        name: "list_dir",
        func: file_list_dir,
    },
    StdlibExport {
        name: "walk_dir",
        func: file_walk_dir,
    },
    StdlibExport {
        name: "copy_file",
        func: file_copy_file,
    },
    StdlibExport {
        name: "move_file",
        func: file_move_file,
    },
    StdlibExport {
        name: "delete_file",
        func: file_delete_file,
    },
    StdlibExport {
        name: "delete_dir",
        func: file_delete_dir,
    },
    StdlibExport {
        name: "temp_file",
        func: file_temp_file,
    },
    StdlibExport {
        name: "temp_dir",
        func: file_temp_dir,
    },
    StdlibExport {
        name: "glob_files",
        func: file_glob_files,
    },
    StdlibExport {
        name: "grep_files",
        func: file_grep_files,
    },
];
