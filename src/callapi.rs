//! Calling a command by name from inside another command, whatever either
//! is written in: the shared core of Python's `flowlang.call` and
//! JavaScript's `flowlang.call`.
//!
//! Names resolve exactly, or with `-` read as `_` (as in the Rust `api`),
//! so `my_lib.my_ctl.my_cmd` reaches `my-lib:my-ctl:my-cmd`. A command's
//! value is its `a` (a flow's single output, or all of its outputs as an
//! object); an error envelope - an object whose `status` is `"err"` - is a
//! failure.

use ndata::data::Data;
use ndata::dataobject::DataObject;

use crate::pycmd::Value;


pub enum CallFailure {
    /// The arguments do not fit the declared parameters.
    BadArgs(String),
    /// The command raised, panicked or returned an error envelope.
    Failed(String),
}

/// A resolved command: its library, id, `lib:ctl:cmd` label and declared
/// parameter names in order.
pub struct Target {
    pub lib: String,
    pub id: String,
    pub label: String,
    pub params: Vec<String>,
}

pub fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() { s.to_string() }
    else if let Some(s) = p.downcast_ref::<String>() { s.clone() }
    else { "unknown panic".to_string() }
}

pub fn resolve_command(lib: &str, ctl: &str, cmd: &str) -> Result<Target, String> {
    let label = format!("{}:{}:{}", lib, ctl, cmd);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resolve_inner(lib, ctl, cmd)))
        .unwrap_or_else(|p| Err(panic_text(&p)))
        .map_err(|why| format!("no command {}: {}", label, why))
}

fn resolve_inner(lib: &str, ctl: &str, cmd: &str) -> Result<Target, String> {
    let store = crate::DataStore::new();
    let safe = |s: &str| s.replace('-', "_");
    // An exact name first, then one whose `-`s read as `_`.
    fn pick(names: Vec<(String, String)>, want: &str, safe: &dyn Fn(&str) -> String) -> Option<(String, String)> {
        names.iter().find(|(n, _)| n == want).cloned()
            .or_else(|| names.iter().find(|(n, _)| safe(n) == want).cloned())
    }
    let libs: Vec<(String, String)> = std::fs::read_dir(&store.root).map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| store.exists(n, "controls"))
        .map(|n| (n.clone(), n))
        .collect();
    let (lib_name, _) = pick(libs, lib, &safe).ok_or("no such library")?;
    let entries = |list: ndata::dataarray::DataArray| -> Vec<(String, String)> {
        list.objects().into_iter().map(|d| {
            let o = d.object();
            (o.get_string("name"), o.get_string("id"))
        }).collect()
    };
    let controls = store.get_data(&lib_name, "controls").get_object("data").get_array("list");
    let (ctl_name, ctl_id) = pick(entries(controls), ctl, &safe).ok_or("no such control")?;
    let ctl_data = store.get_data(&lib_name, &ctl_id).get_object("data");
    if !ctl_data.has("cmd") {
        return Err("the control has no commands".to_string());
    }
    let (cmd_name, cmd_id) = pick(entries(ctl_data.get_array("cmd")), cmd, &safe).ok_or("no such command")?;
    let command = crate::command::Command::new(&lib_name, &cmd_id);
    Ok(Target {
        label: format!("{}:{}:{}", lib_name, ctl_name, cmd_name),
        params: command.params.iter().map(|p| p.0.clone()).collect(),
        lib: lib_name,
        id: cmd_id,
    })
}

/// Runs a resolved command and returns its value as an owned `Value`.
pub fn run_command(target: &Target, args: DataObject) -> Result<Value, CallFailure> {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let label = &target.label;
    let command = catch_unwind(AssertUnwindSafe(|| crate::command::Command::new(&target.lib, &target.id)))
        .map_err(|p| CallFailure::Failed(format!("{} could not be loaded: {}", label, panic_text(&p))))?;
    let a2 = args.clone();
    match catch_unwind(AssertUnwindSafe(|| command.try_cast_params(a2))) {
        Ok(Ok(())) => {}
        Ok(Err(m)) => return Err(CallFailure::BadArgs(format!("arguments for {} do not match its declared parameter types: {}", label, m))),
        Err(p) => return Err(CallFailure::BadArgs(format!("arguments for {} do not match its declared parameter types: {}", label, panic_text(&p)))),
    }
    let result = match catch_unwind(AssertUnwindSafe(|| command.execute(args))) {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(CallFailure::Failed(format!("{}: {}", label, e))),
        Err(p) => return Err(CallFailure::Failed(format!("{} panicked: {}", label, panic_text(&p)))),
    };
    let value = if command.lang == "flow" {
        let keys = result.clone().keys();
        if keys.len() == 1 { Value::from_data(result.get_property(&keys[0])) } else { Value::Object(result) }
    } else if result.has("a") {
        Value::from_data(result.get_property("a"))
    } else {
        Value::Object(result)
    };
    if let Value::Object(o) = &value {
        if let Ok(Data::DString(status)) = o.try_get_property("status") {
            if status == "err" {
                let msg = match o.try_get_property("msg") {
                    Ok(Data::DString(m)) => m,
                    Ok(other) => format!("{:?}", other),
                    Err(_) => "(no msg)".to_string(),
                };
                return Err(CallFailure::Failed(format!("{}: {}", label, msg)));
            }
        }
    }
    Ok(value)
}
