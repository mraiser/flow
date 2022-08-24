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
  pub name: String,
  pub lib: String,
  pub id: String,
  pub lang: String,
  pub src: Source,
  pub return_type: String,
  pub params: Vec<(String, String)>,
}

impl Command {
  pub fn new(lib:&str, id:&str) -> Command {


    // FIXME - support other languages

    let store = DataStore::new();
    let src = store.get_data(lib, id);
    let data = src.get_object("data");
    let typ = &data.get_string("type");
    let name = &data.get_string("name");
    
    let codename = &data.get_string(typ);
    let code = store.get_data(lib, codename).get_object("data");
    let ret = &code.get_string("returntype");
    
    let p = &code.get_array("params");
    let mut params: Vec<(String, String)> = Vec::new();
    for d in p.objects() {
      let d = d.object();
      let a = d.get_string("name");
      let b = d.get_string("type");
      params.push((a,b));
    }
    
    let code = match typ.as_ref() {
      "flow" => {
        let s = code.get_object("flow");
        let case = Case::from_data(s);
        Source::Flow(case)
      },
      "rust" => {
        let codename:&str = &data.get_string("rust");
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

    DataStore::gc();
    
    return Command {
      name: name.to_string(),
      lib: lib.to_string(),
      id: id.to_string(),
      lang: typ.to_string(),
      src: code, 
      return_type: ret.to_string(),
      params: params,
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
  
  pub fn cast_params(&self, mut params:DataObject) {
    for p in &self.params {
      let n = &p.0;
      let t = &p.1;
      if t == "Integer" { params.put_i64(&n, params.get_string(&n).parse::<i64>().unwrap()); }
      else if t == "Float" { params.put_float(&n, params.get_string(&n).parse::<f64>().unwrap()); }
      else if t == "Boolean" { params.put_bool(&n, params.get_string(&n).parse::<bool>().unwrap()); }
      else if t == "JSONObject" { params.put_object(&n, DataObject::from_string(&params.get_string(&n))); }
      else if t == "JSONArray" { params.put_array(&n, DataArray::from_string(&params.get_string(&n))); }
      else { params.put_str(&n, &params.get_string(&n)); }
    }
  }
  
  pub fn execute(&self, args: DataObject) -> Result<DataObject, CodeException> {
    if let Source::Flow(f) = &self.src { 
      let mut code = Code::new(f.duplicate());
      //println!("executing: {:?}", self.src);
      let o = code.execute(args);
      DataStore::gc();
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

