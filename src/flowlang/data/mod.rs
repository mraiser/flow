pub mod read;
pub mod exists;
pub mod library_exists;
pub mod library_new;
pub mod write;
pub mod root;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("psnvun182883c2b36y7e5".to_string(), root::execute, "".to_string()));
    cmds.push(("jqgjwp181688871c7i13".to_string(), write::execute, "".to_string()));
    cmds.push(("gzkkqu1816853499dm1c".to_string(), library_new::execute, "".to_string()));
    cmds.push(("jxmsip181684eaf0ek19".to_string(), library_exists::execute, "".to_string()));
    cmds.push(("hpmngz181683bb1e2s1c".to_string(), exists::execute, "".to_string()));
    cmds.push(("ymrlgs181586ccde7o1c".to_string(), read::execute, "".to_string()));
}
