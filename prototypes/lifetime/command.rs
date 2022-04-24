use crate::code::*;
use crate::dataobject::*;
use crate::flowenv::FlowEnv;

#[derive(Debug)]
pub struct Command<'a> {
  pub lib: String,
  pub id: String,
  pub src: DataObject<'a>,
  pub env: &'a FlowEnv,
}

impl<'a> Command<'a> {
  pub fn new(lib:&str, id:&str, env:&'a FlowEnv) -> Command<'a> {


    // FIXME - support other languages


    let src = env.store.get_data(lib, id, env);
    let data = src.get_object("data");
    let codename = data.get_string("flow");
    let codeval = env.store.get_data(lib, &codename, env).get_object("data").get_object("flow");
    return Command {
      lib: lib.to_string(),
      id: id.to_string(),
      src: codeval,
      env: env,
    };
  }
  
  pub fn execute(&'a self, args: DataObject<'a>) -> Result<DataObject<'a>, CodeException> {
    let mut code = Code::new(self.src.duplicate(), self.env);
    //println!("executing: {:?}", self.src);
    code.execute(args)
  }
}

#[test]
fn verify_test() {
  let path = Path::new("data");
  let store = DataStore::new(path.to_path_buf());
  let command = Command::new("testflow", "zkuwhn1802d57cb8ak1c", store);
  let id = command.id;
  assert_eq!("zkuwhn1802d57cb8ak1c", id);
}

