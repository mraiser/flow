use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::collections::HashMap;
use crate::DataStore;
use ndata::dataobject::DataObject;
use std::sync::Once;
use std::sync::RwLock;
use std::collections::hash_map::DefaultHasher;
use core::hash::Hash;
use std::hash::Hasher;
use crate::pycmd::PyCmd;

static START: Once = Once::new();
static PYENV:RwLock<Option<PyEnv>> = RwLock::new(None);

pub struct PyEnv {
  register: Py<PyAny>,
  exec: Py<PyAny>,
  map: HashMap<String, u64>,
}

impl PyEnv {
  pub fn new() -> PyEnv {
    Python::with_gil(|py| {
      let code = PyModule::from_code(
          py,
          "import json
NNAPI = {}
try:
  import importlib.util
  def loadpython(module, path):
    spec = importlib.util.spec_from_file_location(module, path)
    foo = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(foo)
    return foo
except:
  import imp
  def loadpython(module, path):
    return imp.load_source(module, path)

def register(lib, ctl, cmd, id, path):
  if not lib in NNAPI:
    NNAPI[lib] = {}
  if not ctl in NNAPI[lib]:
    NNAPI[lib][ctl] = {}
  module = 'robot.'+lib+'.'+id
  claz = loadpython(module, path)
  NNAPI[lib][ctl][cmd] = claz.execute

def execute(lib, ctl, cmd, args):
  try:
    res = NNAPI[lib][ctl][cmd](json.loads(args));
    if (type(res) is str):
      d = {
        'status': 'ok',
        'msg': res
      }
    else:
      d = {
        'status': 'ok',
        'data': str(res)
      }
    return json.dumps(d)
  except Exception as err:
    d = {
      'status': 'err',
      'data': str(err)
    }
    return json.dumps(d)",
            "",
            "",
      ).unwrap();
      let register: Py<PyAny> = code.getattr("register").unwrap().into();
      let exec: Py<PyAny> = code.getattr("execute").unwrap().into();
      PyEnv { 
        register: register,
        exec: exec,
        map: HashMap::new(),
      }
    })
  }
  
  pub fn register(&self, lib:&str, ctl:&str, cmd:&str, id:&str, path:&str) {
    Python::with_gil(|py| {
      let args = (lib, ctl, cmd, id, path);
      let res = self.register.call1(py, args);
      if res.is_err() { panic!("{:?} - {}", res, path); }
    });
  }
  
  pub fn execute(&self, lib:&str, ctl:&str, cmd:&str, args:&str) -> String {
    Python::with_gil(|py| {
      let a = (lib, ctl, cmd, args);
      let res = self.exec.call1(py, a);
      if res.is_err() { return format!("{:?} - {:?}", res, a); }
      let res:String = res.unwrap().extract(py).unwrap();
      res 
    })
  }
}

pub fn dopy(lib:&str, id:&str, args:DataObject) -> DataObject{
    #[cfg(not(feature="python_no_singleton"))]
    {
        START.call_once(|| {
          *PYENV.write().unwrap() = Some(PyEnv::new());
        });
    }
    
    let f = PyCmd::get_path(lib, id);
    
    let store = DataStore::new();
    let cmd = store.get_data(lib, id);
    let cmd = cmd.get_object("data");
    let jsid = cmd.get_string("python");
    let name = cmd.get_string("name");
    let cmd = store.get_data(lib, &jsid);    
    let cmd = cmd.get_object("data");
//    println!("{}", cmd.to_json());
    let code = cmd.get_string("python");
    let ctl = cmd.get_string("ctl");
//    let returntype = cmd.get_string("returntype");
    let params = cmd.get_array("params");
    
    let mut a = DataObject::new();
    for o in params.objects(){
      let key = o.object().get_string("name");
      let val = args.get_property(&key);
      a.set_property(&key, val);
    }
    
    // FIXME - Use timestamp instead
    let h1 = calculate_hash(&code);
    #[cfg(not(feature="python_no_singleton"))]
    let wrap = &mut PYENV.write().unwrap();
    #[cfg(not(feature="python_no_singleton"))]
    let wrap = wrap.as_mut().unwrap();
    #[cfg(feature="python_no_singleton")]
    let mut wrap = PyEnv::new();
    let hasfunc;
    let mut h2 = 0;
    {
      let cmdname = "NNAPI.".to_string()+lib+"."+(&ctl)+"."+&name;
      let functions = &mut wrap.map;
      let h3 = functions.get(&cmdname);
      hasfunc = h3.is_some();
      if hasfunc { h2 = *h3.unwrap(); }
      functions.insert(cmdname.to_owned(), h1);
    }

    if !hasfunc || h2 != h1 {
      wrap.register(lib, &ctl, &name, id, &f);
    }
    let res = wrap.execute(lib, &ctl, &name, &a.to_string());
    if !res.starts_with("{"){
      let mut jo = DataObject::new();
      jo.put_string("status", "err");
      jo.put_string("msg", &res);
      return jo;
    }
    DataObject::from_string(&res)
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
  let mut s = DefaultHasher::new();
  t.hash(&mut s);
  s.finish()
}
