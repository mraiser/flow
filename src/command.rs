use ndata::dataobject::*;
use ndata::dataarray::*;
use ndata::data::Data;

use crate::code::*;
use crate::datastore::*;
use crate::case::*;
use crate::rustcmd::*;

#[cfg(feature="java_runtime")]
use crate::javacmd::*;
#[cfg(feature="javascript_runtime")]
use crate::jscmd::*;
use crate::pycmd::*;

#[derive(Debug)]
pub enum Source {
  Flow(Case),
  Rust(RustCmd),
  #[cfg(feature="java_runtime")]
  Java(JavaCmd),
  #[cfg(feature="javascript_runtime")]
  JavaScript(JSCmd),
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
  pub readers: Vec<String>,
  pub writers: Vec<String>,
}

impl Command {
  pub fn exists(lib:&str, id:&str) -> bool {
    let store = DataStore::new();
    let src = store.get_data(lib, id);
    let data = src.get_object("data");
    let typ = &data.get_string("type");
    let codename = &data.get_string(typ);
    let code = store.get_data(lib, codename).get_object("data");
    match typ.as_ref() {
      "flow" => {
        return code.has("flow");
      },
      "rust" => {
        return RustCmd::exists(codename);
      },
      #[cfg(feature="java_runtime")]
      "java" => {
        // FIXME
        return true;
      },
      #[cfg(feature="javascript_runtime")]
      "js" => {
        // FIXME
        return true;
      },
      "python" => {
        // FIXME
        return true;
      },
      _ => panic!("Unknown command type {}", typ),
    };
  }

  pub fn new(lib:&str, id:&str) -> Command {
    let store = DataStore::new();
    let src = store.get_data(lib, id);
    let mut readers = Vec::new();
    let mut writers = Vec::new();
    if src.has("readers") { 
      for r in src.get_array("readers").objects() { readers.push(r.string()); }
    }
    if src.has("writers") { 
      for w in src.get_array("writers").objects() { writers.push(w.string()); }
    }
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
      readers: readers,
      writers: writers,
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
      if params.has(&n) {
        let d = params.get_property(&n);
        if t == "Integer" { 
          if d.is_int() { params.put_int(&n, d.int()); }
          else { params.put_int(&n, Data::as_string(d).parse::<i64>().unwrap()); }
        }
        else if t == "Float" { 
          if d.is_float() { params.put_float(&n, d.float()); }
          else { params.put_float(&n, Data::as_string(d).parse::<f64>().unwrap()); }
        }
        else if t == "Boolean" { 
          if d.is_boolean() { params.put_boolean(&n, d.boolean()); }
          else { params.put_boolean(&n, Data::as_string(d).parse::<bool>().unwrap()); }
        }
        else if t == "JSONObject" { 
          if d.is_object() { params.put_object(&n, d.object()); }
          else { params.put_object(&n, DataObject::from_string(&Data::as_string(d))); }
        }
        else if t == "JSONArray" { 
          if d.is_array() { params.put_array(&n, d.array()); }
          else { params.put_array(&n, DataArray::from_string(&Data::as_string(d))); }
        }
        else { 
          if d.is_string() { params.put_string(&n, &d.string()); }
          else { params.put_string(&n, &Data::as_string(d)); }
        }
      }
      else if t == "Any" { params.put_null(&n); }
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

