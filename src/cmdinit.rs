use crate::flowlang;
use crate::mcp;
use crate::rustcmd::*;
use crate::testflow;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    flowlang::cmdinit(cmds);
    mcp::cmdinit(cmds);
    testflow::cmdinit(cmds);
}
