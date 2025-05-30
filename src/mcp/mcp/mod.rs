pub mod list_resources;
pub mod describe;
pub mod initialize;
pub mod invoke;
pub mod list_prompts;
pub mod list_tools;
pub mod mcp;

pub fn cmdinit(cmds: &mut Vec<(String, crate::rustcmd::Transform, String)>) {
    cmds.push(("mrnvko1968170c5d3zfc".to_string(), mcp::execute, "".to_string()));
    cmds.push(("jkhjhj196819a041ao15c".to_string(), list_tools::execute, "".to_string()));
    cmds.push(("owqqtr196898cd18eibb".to_string(), list_prompts::execute, "".to_string()));
    cmds.push(("jnsipy19682ab0d8aw211".to_string(), invoke::execute, "".to_string()));
    cmds.push(("sqtvpk19682925f30n1d8".to_string(), initialize::execute, "".to_string()));
    cmds.push(("lxmotl19682036095q9b".to_string(), describe::execute, "".to_string()));
    cmds.push(("pnzmqz196898e7fdekc1".to_string(), list_resources::execute, "".to_string()));
}
