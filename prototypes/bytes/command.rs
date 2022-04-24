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
//    println!("executing: {:?}", self.src);
    let mut code = Code::new(self.src.duplicate(), self.store.clone());
    code.execute(args)
  }
}


