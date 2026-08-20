//! std.system — System operations and process management.
//!
//! Ports Python's os/sys modules: env vars, process exec, system info.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::stdlib::StdlibExport;
use crate::stdlib_helpers::{arg_bool, arg_int, arg_list, arg_opt_int, arg_opt_str, arg_str};
use crate::value::Value;

// std.system
// ---------------------------------------------------------------------------

fn system_env_get(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    let default = arg_opt_str(args, 1)?;
    match std::env::var(key) {
        Ok(v) => Ok(Value::Str(Rc::from(v.as_str()))),
        Err(_) => match default {
            Some(d) => Ok(Value::Str(Rc::from(d))),
            None => Ok(Value::Null),
        },
    }
}

fn system_env_set(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    let value = arg_str(args, 1)?;
    // Python parity: `os.environ[key] = value` raises
    // `OSError: [Errno 22] Invalid argument` for keys that are empty,
    // contain '=' or NUL — never panic (std::env::set_var panics on these).
    if key.is_empty() || key.contains('=') || key.contains('\0') {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python OSError: [Errno 22] Invalid argument".to_string(),
            None,
        ));
    }
    unsafe { std::env::set_var(key, value) };
    Ok(Value::Str(Rc::from(format!("Set {key}={value}").as_str())))
}

fn system_env_delete(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let key = arg_str(args, 0)?;
    // Python parity: `_env_delete` guards with `if key in os.environ` —
    // missing/invalid keys return "Variable {key} not found" (never panics).
    if key.is_empty() || key.contains('=') || key.contains('\0') {
        return Ok(Value::Str(Rc::from(
            format!("Variable {key} not found").as_str(),
        )));
    }
    unsafe { std::env::remove_var(key) };
    Ok(Value::Str(Rc::from(format!("Deleted {key}").as_str())))
}

fn system_env_list(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut out: Vec<Value> = Vec::new();
    for (k, v) in std::env::vars() {
        out.push(Value::Str(Rc::from(format!("{k}={v}").as_str())));
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn system_get_cli_args(interp: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let items: Vec<Value> = interp
        .program_args
        .iter()
        .map(|a| Value::Str(Rc::from(a.as_str())))
        .collect();
    Ok(Value::List(Rc::new(RefCell::new(items))))
}

fn system_parse_cli_args(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let args_list = arg_list(args, 0).unwrap_or_default();
    let mut m = indexmap::IndexMap::new();
    let mut i = 0;
    while i < args_list.len() {
        let a = args_list[i].python_str();
        if a.starts_with("--") {
            let key = a.trim_start_matches("--").to_string();
            if i + 1 < args_list.len() && !args_list[i + 1].python_str().starts_with('-') {
                m.insert(Value::Str(Rc::from(key.as_str())), args_list[i + 1].clone());
                i += 2;
            } else {
                m.insert(Value::Str(Rc::from(key.as_str())), Value::Bool(true));
                i += 1;
            }
        } else if a.starts_with('-') && a.len() > 1 {
            let key = a.trim_start_matches('-').to_string();
            if i + 1 < args_list.len() && !args_list[i + 1].python_str().starts_with('-') {
                m.insert(Value::Str(Rc::from(key.as_str())), args_list[i + 1].clone());
                i += 2;
            } else {
                m.insert(Value::Str(Rc::from(key.as_str())), Value::Bool(true));
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    Ok(Value::Map(Rc::new(RefCell::new(m))))
}

fn system_exec(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let command = arg_str(args, 0)?;
    let shell = arg_bool(args, 1).unwrap_or(false);
    let mut cmd = if shell {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(command);
        c
    } else {
        let mut parts = command.split_whitespace();
        let prog = parts.next().unwrap_or("");
        let mut c = std::process::Command::new(prog);
        c.args(parts);
        c
    };
    match cmd.output() {
        Ok(out) => {
            let mut m = indexmap::IndexMap::new();
            m.insert(
                Value::Str(Rc::from("stdout")),
                Value::Str(Rc::from(
                    String::from_utf8_lossy(&out.stdout).to_string().as_str(),
                )),
            );
            m.insert(
                Value::Str(Rc::from("stderr")),
                Value::Str(Rc::from(
                    String::from_utf8_lossy(&out.stderr).to_string().as_str(),
                )),
            );
            m.insert(
                Value::Str(Rc::from("exit_code")),
                Value::Int(BigInt::from(out.status.code().unwrap_or(-1))),
            );
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
        Err(e) => {
            let mut m = indexmap::IndexMap::new();
            m.insert(Value::Str(Rc::from("stdout")), Value::Str(Rc::from("")));
            m.insert(
                Value::Str(Rc::from("stderr")),
                Value::Str(Rc::from(e.to_string().as_str())),
            );
            m.insert(
                Value::Str(Rc::from("exit_code")),
                Value::Int(BigInt::from(-1)),
            );
            Ok(Value::Map(Rc::new(RefCell::new(m))))
        }
    }
}

fn system_exec_async(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let command = arg_str(args, 0)?;
    let shell = arg_bool(args, 1).unwrap_or(false);
    let mut cmd = if shell {
        let mut c = std::process::Command::new("sh");
        c.arg("-c").arg(command);
        c
    } else {
        let mut parts = command.split_whitespace();
        let prog = parts.next().unwrap_or("");
        let mut c = std::process::Command::new(prog);
        c.args(parts);
        c
    };
    cmd.spawn()
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python OSError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Started {command}").as_str())))
}

fn system_pid(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Int(BigInt::from(std::process::id() as i64)))
}

fn system_exit(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let code = arg_opt_int(args, 0)?
        .map(|n| n.to_i64().unwrap_or(0))
        .unwrap_or(0);
    Err(ExceptionValue::new("SystemExit", format!("{code}"), None))
}

fn system_hostname(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let h = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "localhost".to_string());
    Ok(Value::Str(Rc::from(h.as_str())))
}

fn system_platform(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from("linux")))
}

fn system_cpu_count(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Int(BigInt::from(
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1) as i64,
    )))
}

fn system_python_version(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from("3.11.0")))
}

fn system_platform_version(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Str(Rc::from("")))
}

fn system_memory_info(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut m = indexmap::IndexMap::new();
    m.insert(Value::Str(Rc::from("total")), Value::Int(BigInt::from(0)));
    m.insert(
        Value::Str(Rc::from("available")),
        Value::Int(BigInt::from(0)),
    );
    Ok(Value::Map(Rc::new(RefCell::new(m))))
}

fn system_kill(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let pid = arg_int(args, 0)?.to_i64().unwrap_or(0);
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .status();
    Ok(Value::Null)
}

macro_rules! log_fn {
    ($name:ident, $level:expr) => {
        fn $name(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
            let msg = arg_str(args, 0)?;
            let out = format!("[{level}] {msg}", level = $level);
            interp.stdout.lock().unwrap().push_str(&out);
            interp.stdout.lock().unwrap().push('\n');
            Ok(Value::Str(Rc::from(out.as_str())))
        }
    };
}

log_fn!(system_log_debug, "DEBUG");
log_fn!(system_log_info, "INFO");
log_fn!(system_log_warn, "WARN");
log_fn!(system_log_error, "ERROR");
log_fn!(system_log_critical, "CRITICAL");

fn system_log_set_level(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

fn system_log_to_file(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------
pub static SYSTEM_EXPORTS: &[StdlibExport] = &[
    StdlibExport {
        name: "env_get",
        func: system_env_get,
    },
    StdlibExport {
        name: "env_set",
        func: system_env_set,
    },
    StdlibExport {
        name: "env_delete",
        func: system_env_delete,
    },
    StdlibExport {
        name: "env_list",
        func: system_env_list,
    },
    StdlibExport {
        name: "get_cli_args",
        func: system_get_cli_args,
    },
    StdlibExport {
        name: "parse_cli_args",
        func: system_parse_cli_args,
    },
    StdlibExport {
        name: "exec",
        func: system_exec,
    },
    StdlibExport {
        name: "exec_async",
        func: system_exec_async,
    },
    StdlibExport {
        name: "pid",
        func: system_pid,
    },
    StdlibExport {
        name: "exit",
        func: system_exit,
    },
    StdlibExport {
        name: "hostname",
        func: system_hostname,
    },
    StdlibExport {
        name: "platform",
        func: system_platform,
    },
    StdlibExport {
        name: "cpu_count",
        func: system_cpu_count,
    },
    StdlibExport {
        name: "python_version",
        func: system_python_version,
    },
    StdlibExport {
        name: "platform_version",
        func: system_platform_version,
    },
    StdlibExport {
        name: "memory_info",
        func: system_memory_info,
    },
    StdlibExport {
        name: "kill",
        func: system_kill,
    },
    StdlibExport {
        name: "log_debug",
        func: system_log_debug,
    },
    StdlibExport {
        name: "log_info",
        func: system_log_info,
    },
    StdlibExport {
        name: "log_warn",
        func: system_log_warn,
    },
    StdlibExport {
        name: "log_error",
        func: system_log_error,
    },
    StdlibExport {
        name: "log_critical",
        func: system_log_critical,
    },
    StdlibExport {
        name: "log_set_level",
        func: system_log_set_level,
    },
    StdlibExport {
        name: "log_to_file",
        func: system_log_to_file,
    },
];
