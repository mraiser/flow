pub mod plus;
pub mod minus;
pub mod less_than;
pub mod multiply;
pub mod divide;
pub mod or;
pub mod greater_than;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("ntgnjv181638a6a1fr13".to_string(), greater_than::execute, "".to_string()));
    cmds.push(("nnmpzy1816382fc10w25".to_string(), or::execute, "".to_string()));
    cmds.push(("ohtnlg1813f03925fv1f".to_string(), divide::execute, "".to_string()));
    cmds.push(("guulgu1813f025c7aw1c".to_string(), multiply::execute, "".to_string()));
    cmds.push(("uwjgmz180e74b084el1b".to_string(), less_than::execute, "".to_string()));
    cmds.push(("uhtyts180ce1ad355u1b9".to_string(), minus::execute, "".to_string()));
    cmds.push(("xuxqyr180cd72e058m199".to_string(), plus::execute, "".to_string()));
}
