pub mod listen;
pub mod accept;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("jxqtnn1813e80eb63v12".to_string(), accept::execute, "".to_string()));
    cmds.push(("vipnih1813e77c034v1d".to_string(), listen::execute, "".to_string()));
}
