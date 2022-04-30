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
  pub fn new(lib:&str, id:&str) -> Command {


    // FIXME - support other languages

    let env = &mut FLOWENV.get().write().unwrap();
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
  
  pub fn execute(&self, args: DataObject) -> Result<DataObject, CodeException> {
    let mut code = Code::new(self.src.duplicate());
    //println!("executing: {:?}", self.src);
    let o = code.execute(args);
    let env = &mut FLOWENV.get().write().unwrap();
    env.gc();
    o
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::Path;
  use crate::datastore::*;
  use std::sync::Once;

  static INIT: Once = Once::new();

  pub fn initialize() {
    INIT.call_once(|| {
      let path = Path::new("data");
      let store = DataStore::new(path.to_path_buf());
      
      FlowEnv::init(store);
    });
  }

  #[test]
  fn test_add(){
    initialize();

    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 299,
      "b": 121
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "zkuwhn1802d57cb8ak1c");
    let res = cmd.execute(args).unwrap();
    let a = res.get_string("a");
    assert_eq!(a, "299+121=420");
  }

  #[test]
  fn test_command() {
    initialize();

    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 210
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "vnpvxv1802d67b7d1j1f");
    let res = cmd.execute(args).unwrap();
    let a = res.get_string("a");
    assert_eq!(a, "210+210=420");
  }

  #[test]
  fn test_conditionals() {
    initialize();
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": true
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "ooizjt1803765b08ak212");
    let res = cmd.execute(args).unwrap();
    let a = res.get_i64("a");
    assert_eq!(a, 2);
  }

  #[test]
  fn test_lists() {
    initialize();
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": [1,2,3]
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "izzpiy1803778a841p3a5");
    let res = cmd.execute(args).unwrap();
    let a = res.get_array("a").to_json().to_string();
    assert_eq!(a, "[2,3,4]");
  }

  #[test]
  fn test_loop() {
    initialize();
    
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 0
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "izmuzm18037d796f1i467");
    let res = cmd.execute(args).unwrap();
    let a = res.get_i64("a");
    assert_eq!(a, 4);
  }

  #[test]
  fn test_speed() {
    initialize();
     
    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 1000
    }
    "#).unwrap());
    let cmd = Command::new("testflow", "jqlvrz18041a69d0bw311");
    let res = cmd.execute(args).unwrap();
    let a = res.get_i64("a");
    assert!(a>0);
 }
}
