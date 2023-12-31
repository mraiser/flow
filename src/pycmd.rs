use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use crate::code::CodeException;
use crate::DataStore;
use crate::flowlang::system::system_call::system_call;

#[cfg(feature="python_runtime")]
use crate::pyenv::*;

#[derive(Debug)]
pub struct PyCmd {
  #[cfg(not(feature="python_runtime"))]
  path:String,
  #[cfg(feature="python_runtime")]
  lib:String,
  #[cfg(feature="python_runtime")]
  id:String,
}

impl PyCmd{
  pub fn new(lib:&str, id:&str) -> PyCmd{
    PyCmd{
      #[cfg(not(feature="python_runtime"))]
      path: PyCmd::get_path(lib,id),
      #[cfg(feature="python_runtime")]
      lib:lib.to_string(),
      #[cfg(feature="python_runtime")]
      id:id.to_string(),
    }
  }
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    #[cfg(not(feature="python_runtime"))]
    {
        let mut a = DataArray::new();
        a.push_string("python");
        a.push_string(&self.path);
        a.push_string(&args.to_string());
        let o = system_call(a);
        let err = o.get_string("err");
        if &err == "" {
            let out = o.get_string("out");
            let mut jo = DataObject::new();
            jo.put_string("msg", &out);
            return Ok(jo);
        }
        else {
            let mut jo = DataObject::new();
            jo.put_string("status", "err");
            jo.put_string("msg", &err);
            return Ok(jo);
        }
    }
    #[cfg(feature="python_runtime")]
    Ok(dopy(&self.lib, &self.id, args))
  }
  
  pub fn get_path(lib:&str, id:&str) -> String {
    let store = DataStore::new();
    let data = store.get_data(lib, id);
    let data = data.get_object("data");
    let data = data.get_string("python");
    let data = store.get_data(lib, &data);
    let data = data.get_object("data");
    let ctl = data.get_string("ctl");
    let cmd = data.get_string("cmd");
    let root = store.get_lib_root(&lib);
    let filename = cmd.clone()+".py";
    let path = root.join("src").join(&lib).join(&ctl).join(&filename);    
    path.display().to_string()
  }
}
