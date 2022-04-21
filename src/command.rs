use crate::code::*;
use crate::dataobject::*;
use crate::datastore::DataStore;

#[derive(Debug)]
pub struct Command {
  pub lib: String,
  pub id: String,
  pub src: DataObject,
  pub store: DataStore,
}

impl Command {
  pub fn new(lib:&str, id:&str, store:DataStore) -> Command {


    // FIXME - support other languages


    let src = store.get_data(lib, id);
    let data = src.get_object("data");
    let codename = data.get_string("flow");
    let codeval = store.get_data(lib, &codename).get_object("data").get_object("flow");
    return Command {
      lib: lib.to_string(),
      id: id.to_string(),
      src: codeval,
      store: store,
    };
  }
  
  pub fn execute(&self, args: DataObject) -> Result<DataObject, CodeException> {
    let mut code = Code::new(self.src.duplicate(), self.store.clone());
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

