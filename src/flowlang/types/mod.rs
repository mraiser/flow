pub mod to_int;
pub mod to_float;
pub mod to_boolean;
pub mod to_string;
pub mod is_string;
pub mod is_object;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("tjhuni18283434ae2k178c".to_string(), is_object::execute, "".to_string()));
    cmds.push(("yunyvp1825eaa551fn16".to_string(), is_string::execute, "".to_string()));
    cmds.push(("xzhuqo1825a3a8102x1d".to_string(), to_string::execute, "".to_string()));
    cmds.push(("wskvnq1825a38c770w1a".to_string(), to_boolean::execute, "".to_string()));
    cmds.push(("npzhil1825a37d602p17".to_string(), to_float::execute, "".to_string()));
    cmds.push(("lmvhmw1825a03e3e3o1e".to_string(), to_int::execute, "".to_string()));
}
