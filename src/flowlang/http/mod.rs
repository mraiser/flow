pub mod listen;
pub mod hex_decode;
pub mod cast_params;
pub mod websocket;
pub mod websocket_read;
pub mod websocket_write;
pub mod hex_encode;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("mwtghp182164edb0bk19".to_string(), websocket_write::execute, "".to_string()));
    cmds.push(("ozpqhh1820d669701i25".to_string(), websocket_read::execute, "".to_string()));
    cmds.push(("pkgvku1820d2f0974y22".to_string(), websocket::execute, "".to_string()));
    cmds.push(("vqllhi180d2ac7398m50".to_string(), listen::execute, "".to_string()));
    cmds.push(("zinilr182eee5a81as7c2".to_string(), hex_encode::execute, "".to_string()));
    cmds.push(("pzhyll180d377c16coac".to_string(), hex_decode::execute, "".to_string()));
    cmds.push(("zvrrxm180ec79ad12u36".to_string(), cast_params::execute, "".to_string()));
}
