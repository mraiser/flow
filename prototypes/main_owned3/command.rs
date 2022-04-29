use crate::code::*;
use crate::dataobject::*;
use crate::flowenv::*;
use crate::case::*;

#[derive(Debug)]
pub struct Command {
  pub lib: String,
  pub id: String,
  pub src: Case,
}

impl Command {
  pub fn new(lib:&str, id:&str, env:&mut FlowEnv) -> Command {


    // FIXME - support other languages

    let store = &mut env.store.clone();
    let src = store.get_json(lib, id);
    let data = &src["data"];
    let codename:&str = data["flow"].as_str().unwrap();
    
    let code = store.get_code(lib, codename);
    
//    let codeval = store.get_data(lib, codename, env).get_object("data", env).get_object("flow", env);
    return Command {
      lib: lib.to_string(),
      id: id.to_string(),
      src: code, //codeval,
    };
  }
  
  pub fn execute(&self, args: DataObject, env:&mut FlowEnv) -> Result<DataObject, CodeException> {
    let mut code = Code::new(self.src.duplicate());
    //println!("executing: {:?}", self.src);
    let o = code.execute(args, env);
    env.gc();
    o
  }
}

