use crate::code::*;
use crate::dataobject::*;
use crate::flowenv::*;

#[derive(Debug)]
pub struct Command {
  pub lib: String,
  pub id: String,
  pub src: DataObject,
}

impl Command {
  pub fn new(lib:&str, id:&str, env:&mut FlowEnv) -> Command {


    // FIXME - support other languages

    let store = &mut env.store.clone();
    let src = store.get_data(lib, id, env);
    let data = src.get_object("data", env);
    let codename = data.get_string("flow", env);
    let codeval = store.get_data(lib, &codename, env).get_object("data", env).get_object("flow", env);
    return Command {
      lib: lib.to_string(),
      id: id.to_string(),
      src: codeval,
    };
  }
  
  pub fn execute(&self, args: DataObject, env:&mut FlowEnv) -> Result<DataObject, CodeException> {
    let mut code = Code::new(self.src.duplicate(env));
    //println!("executing: {:?}", self.src);
    let o = code.execute(args, env);
    env.gc();
    o
  }
}

