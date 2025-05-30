pub mod mcp;

pub fn cmdinit(cmds: &mut Vec<(String, crate::rustcmd::Transform, String)>) {
    mcp::cmdinit(cmds);
}
