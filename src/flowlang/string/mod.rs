pub mod split;
pub mod length;
pub mod trim;
pub mod ends_with;
pub mod starts_with;
pub mod left;
pub mod right;
pub mod substring;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("xzhnon181c9ab1ad6i1f".to_string(), substring::execute, "".to_string()));
    cmds.push(("mptiwm181c9a760a9l1c".to_string(), right::execute, "".to_string()));
    cmds.push(("xxivpu181c99f6947n19".to_string(), left::execute, "".to_string()));
    cmds.push(("slhhql181c990bbeev19".to_string(), starts_with::execute, "".to_string()));
    cmds.push(("huzwjx18186d0610cy1a".to_string(), ends_with::execute, "".to_string()));
    cmds.push(("xmiqjx18152d5009ei1a".to_string(), trim::execute, "".to_string()));
    cmds.push(("hghznv180e74646f6m18".to_string(), length::execute, "".to_string()));
    cmds.push(("lyqkxo180e17082e0h8c".to_string(), split::execute, "".to_string()));
}
