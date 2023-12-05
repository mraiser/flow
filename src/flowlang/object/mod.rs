pub mod get;
pub mod to_json;
pub mod has;
pub mod set;
pub mod object_from_json;
pub mod array_from_json;
pub mod keys;
pub mod index_of;
pub mod push;
pub mod push_all;
pub mod remove;
pub mod equals;
pub mod get_or_null;
use crate::rustcmd::*;
pub fn cmdinit(cmds: &mut Vec<(String, Transform, String)>) {
    cmds.push(("qrozil181d99d5231r1c".to_string(), get_or_null::execute, "".to_string()));
    cmds.push(("nmtwuo181d40f7928t1e".to_string(), equals::execute, "".to_string()));
    cmds.push(("titqik18163acb0a2r18".to_string(), remove::execute, "".to_string()));
    cmds.push(("ynxjiz18158f10794j1b".to_string(), push_all::execute, "".to_string()));
    cmds.push(("tpxrsq18158618917k13".to_string(), push::execute, "".to_string()));
    cmds.push(("hxhlqp1815849c941k14".to_string(), index_of::execute, "".to_string()));
    cmds.push(("mgtqsi181582e3e03p1a".to_string(), keys::execute, "".to_string()));
    cmds.push(("hklupg18148ec4702n21".to_string(), array_from_json::execute, "".to_string()));
    cmds.push(("qqksjj18148eab3c7w1d".to_string(), object_from_json::execute, "".to_string()));
    cmds.push(("xtpkyw18148c64416t18".to_string(), set::execute, "".to_string()));
    cmds.push(("kjunxi181399baee0j1d".to_string(), has::execute, "".to_string()));
    cmds.push(("jlxwvg180e78d6843k18".to_string(), to_json::execute, "".to_string()));
    cmds.push(("quxxtx180e1cee57ap19".to_string(), get::execute, "".to_string()));
}
