use crate::flowlang;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    flowlang::cmdinit(cmds);
}
