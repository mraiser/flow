pub mod read_all_string;
pub mod exists;
pub mod read_properties;
pub mod visit;
pub mod is_dir;
pub mod mime_type;
pub mod list;
pub mod write_properties;
pub mod copy_dir;

use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("xrnrjm1835d79671eye".to_string(), copy_dir::execute, "".to_string()));
    cmds.push(("ltkiiu182fa078f53m141e".to_string(), write_properties::execute, "".to_string()));
    cmds.push(("rotplt18186905a73k1e".to_string(), visit::execute, "".to_string()));
    cmds.push(("unsuhp18148f542c7i1a".to_string(), read_properties::execute, "".to_string()));
    cmds.push(("kmxzrv18148b495b6s1e".to_string(), read_all_string::execute, "".to_string()));
    cmds.push(("yypnjg181cfc2da9ez17".to_string(), mime_type::execute, "".to_string()));
    cmds.push(("ohgvku182368963c4s1c".to_string(), list::execute, "".to_string()));
    cmds.push(("swngov181cfa558fdw1a".to_string(), is_dir::execute, "".to_string()));
    cmds.push(("uoyxgz18148b63ccdy21".to_string(), exists::execute, "".to_string()));
}
