pub mod test_rust;

pub fn cmdinit(cmds: &mut Vec<(String, crate::rustcmd::Transform, String)>) {
    cmds.push(("omvgmg1807a950539s96b".to_string(), test_rust::execute, "".to_string()));
}
