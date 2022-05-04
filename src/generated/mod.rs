use crate::rustcmd::*;
pub struct Generated {}
impl Generated {
  pub fn get(name:&str) -> Transform {
    match name {
      _ => { panic!("No such rust command {}", name); }
    }
  }
}
