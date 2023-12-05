pub mod file;
pub mod http;
pub mod math;
pub mod object;
pub mod string;
pub mod system;
pub mod tcp;
pub mod data;
pub mod types;

use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    types::cmdinit(cmds);
    tcp::cmdinit(cmds);
    system::cmdinit(cmds);
    string::cmdinit(cmds);
    object::cmdinit(cmds);
    math::cmdinit(cmds);
    http::cmdinit(cmds);
    file::cmdinit(cmds);
    data::cmdinit(cmds);
}
