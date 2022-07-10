use ndata::dataobject::*;
use ndata::dataarray::*;

use crate::code::*;
use crate::datastore::*;
use crate::case::*;
use crate::rustcmd::*;
use crate::javacmd::*;
use crate::generated::*;

#[derive(Debug)]
pub enum Source {
  Flow(Case),
  Rust(RustCmd),
  Java(JavaCmd),
}

#[derive(Debug)]
pub struct Command {
  pub lib: String,
  pub id: String,
  pub src: Source,
}

impl Command {
  pub fn new(lib:&str, id:&str) -> Command {


    // FIXME - support other languages

    let store = DataStore::new();
    let src = store.get_json(lib, id);
    let data = &src["data"];
    let typ = &data["type"];
    
    let code;
    if typ == "flow" {
      let codename:&str = data["flow"].as_str().unwrap();
      let path = store.get_data_file(lib, &(codename.to_owned()+".flow"));
      let s = store.read_file(path);
      let case = Case::new(&s).unwrap();
      code = Source::Flow(case);
    }
    else if typ == "rust" {
      let codename:&str = data["rust"].as_str().unwrap();
      code = Source::Rust(RustCmd::new(codename));
    }
    else if typ == "java" {
//      let codename:&str = data["java"].as_str().unwrap();
      code = Source::Java(JavaCmd::new(lib, id));
    }
    else { panic!("Unknown command type {}", typ); }
    
    return Command {
      lib: lib.to_string(),
      id: id.to_string(),
      src: code, 
    };
  }
  
  pub fn lookup(lib:&str, ctl:&str, cmd:&str) -> Command {
    let id;
    {
      let store = DataStore::new();
      id = store.lookup_cmd_id(lib, ctl, cmd);
    }
    Command::new(lib, &id)
  }
  
  pub fn execute(&self, args: DataObject) -> Result<DataObject, CodeException> {
    if let Source::Flow(f) = &self.src { 
      let mut code = Code::new(f.duplicate());
      //println!("executing: {:?}", self.src);
      let o = code.execute(args);
      DataObject::gc();
      DataArray::gc();
      return o;
    }
    else if let Source::Rust(r) = &self.src { 
      return r.execute(args);
    }
    else if let Source::Java(r) = &self.src { 
      return r.execute(args);
    }
    else { panic!("Not flow code"); }
  }
  
  pub fn src(&self) -> Case {
    if let Source::Flow(f) = &self.src { f.duplicate() } else { panic!("Not flow code"); }
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
      DataStore::init("data");
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
