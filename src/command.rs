use ndata::dataobject::*;
use ndata::dataarray::*;

use crate::code::*;
use crate::datastore::*;
use crate::case::*;
use crate::rustcmd::*;

#[cfg(feature="java_runtime")]
use crate::javacmd::*;
#[cfg(feature="javascript_runtime")]
use crate::jscmd::*;
#[cfg(feature="python_runtime")]
use crate::pycmd::*;

#[derive(Debug)]
pub enum Source {
  Flow(Case),
  Rust(RustCmd),
  #[cfg(feature="java_runtime")]
  Java(JavaCmd),
  #[cfg(feature="javascript_runtime")]
  JavaScript(JSCmd),
  #[cfg(feature="python_runtime")]
  Python(PyCmd),
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
    let typ = typ.as_str().unwrap();
    
    let code = match typ.as_ref() {
      "flow" => {
        let codename:&str = data["flow"].as_str().unwrap();
        let path = store.get_data_file(lib, &(codename.to_owned()+".flow"));
        let s = store.read_file(path);
        let case = Case::new(&s).unwrap();
        Source::Flow(case)
      },
      "rust" => {
        let codename:&str = data["rust"].as_str().unwrap();
        Source::Rust(RustCmd::new(codename))
      },
      #[cfg(feature="java_runtime")]
      "java" => {
        Source::Java(JavaCmd::new(lib, id))
      },
      #[cfg(feature="javascript_runtime")]
      "js" => {
        Source::JavaScript(JSCmd::new(lib, id))
      },
      #[cfg(feature="python_runtime")]
      "python" => {
        Source::Python(PyCmd::new(lib, id))
      },
      _ => panic!("Unknown command type {}", typ),
    };
    
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
    if let Source::Rust(r) = &self.src {
      return r.execute(args);
    }
    #[cfg(feature="java_runtime")]
    {
      if let Source::Java(r) = &self.src {
        return r.execute(args);
      }
    }
    #[cfg(feature="javascript_runtime")]
    {
      if let Source::JavaScript(r) = &self.src {
        return r.execute(args);
      }
    }
    #[cfg(feature="python_runtime")]
    {
      if let Source::Python(r) = &self.src {
        return r.execute(args);
      }
    }
    panic!("Language not supported: {:?}", &self.src);
  }
  
  pub fn src(&self) -> Case {
    if let Source::Flow(f) = &self.src { f.duplicate() } else { panic!("Not flow code"); }
  }
}

