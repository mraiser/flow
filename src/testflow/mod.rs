pub mod testflow;

pub fn cmdinit(cmds: &mut Vec<(String, crate::rustcmd::Transform, String)>) {
    testflow::cmdinit(cmds);
}
