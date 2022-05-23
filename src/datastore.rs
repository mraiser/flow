use serde_json::*;
use std::fs::File;
use std::path::*;
use std::io::Read;
use std::time::{SystemTime, UNIX_EPOCH};

use ndata::dataobject::*;
use ndata::dataarray::*;

use crate::rand::*;

static mut STORE_PATH:Option<PathBuf> = None;
static mut RANDOM:(u32, u32, u32, u32) = (0,0,0,0);

#[derive(Debug)]
pub struct DataStore {
  pub root: PathBuf,
}

impl DataStore {
  pub fn init(dir:&str) {
    let d = Path::new(dir);

    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .subsec_nanos();
    let rand = Rand::new(nanos);

    unsafe { 
      STORE_PATH = Some(d.to_path_buf()); 
      RANDOM = rand.get();
    }

    DataObject::init();
    DataArray::init();
    let o = DataObject::new();
    let _x = &mut OHEAP.get().write().unwrap().incr(o.data_ref);
  }
  
  pub fn new() -> DataStore {
    let path;
    unsafe {
      path = STORE_PATH.as_ref().unwrap();
    }
    return DataStore {
      root: path.to_path_buf(),
    };
  }
  
  pub fn globals() -> DataObject {
    DataObject::get(0)
  }
  
  pub fn rand() -> u32 {
    unsafe {
      let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
      let x = rand.rand();
      RANDOM = rand.get();
      return x;
    }
  }
  
  pub fn shuffle<T>(a: &mut [T]) {
    unsafe {
      let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
      let o = rand.shuffle(a);
      RANDOM = rand.get();
      return o;
    }
  }

  pub fn rand_range(a: i32, b: i32) -> i32 {
    unsafe {
      let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
      let o = rand.rand_range(a, b);
      RANDOM = rand.get();
      return o;
    }
  }

  pub fn rand_float() -> f64 {
    unsafe {
      let mut rand = Rand::build(RANDOM.0, RANDOM.1, RANDOM.2, RANDOM.3);
      let o = rand.rand_float();
      RANDOM = rand.get();
      return o;
    }
  }

  pub fn lookup_cmd_id(&self, lib:&str, ctl:&str, cmd:&str) -> String {
    let data = self.get_json(lib, "controls");
    let data = &data["data"]["list"];
    for c in data.as_array().unwrap() {
      let n = c["name"].as_str().unwrap();
      if n == ctl {
        let ctlid = c["id"].as_str().unwrap();
        let ctldata = self.get_json(lib, ctlid);
        let ctldata = &ctldata["data"];
        for cmddata in ctldata["cmd"].as_array().unwrap() {
          let n2 = cmddata["name"].as_str().unwrap();
          if n2 == cmd {
            let id = cmddata["id"].as_str().unwrap();
            return id.to_string();
          }
        }
      }
    }
    panic!("No such command {}:{}:{}", lib, ctl, cmd);
  }
    
  pub fn get_json(&self, db: &str, id: &str) -> Value {
    let path = self.get_data_file(db, id);
    let s = self.read_file(path);
    let mut data: Value = serde_json::from_str(&s).unwrap();
    let attachments: Value = data["data"]["attachmentkeynames"].to_owned();
    if attachments.is_array() {
      for a in attachments.as_array().unwrap().into_iter(){
        let b = &a.as_str().unwrap();
        let aid = id.to_string()+"."+b;
        let apath = self.get_data_file(db, &aid);
        let astr = self.read_file(apath);
        if astr.len() > 0 && astr[0..1].to_string() == "{" { // FIXME - Legacy hack
          data["data"][b] = serde_json::from_str(&astr).unwrap(); 
        } else {
          data["data"][b] = serde_json::Value::String(astr); 
        }
      }
    }
    data
  }
  
  pub fn get_data(&self, db: &str, id: &str) -> DataObject {
    let data = self.get_json(db, id);
    DataObject::from_json(data)
  }
  
  pub fn get_data_file(&self, db: &str, id: &str) -> PathBuf {
    let mut path = self.root.join(db);
    path = self.get_sub_dir(path, id, 4, 4);
    path.push(id);
    path
  }
  
  fn get_sub_dir(&self, mut path: PathBuf, id: &str, chars: usize, levels: usize) -> PathBuf {
    let l:usize = chars * levels;
    let mut s = id.to_string();
    while s.len()<l {
      s = s + "_";
    }
    let mut i = 0;
    while i<levels{
      let n:usize = i * chars;
      let m:usize = n + chars;
      i = i + 1;
      let sub = &s[n..m];
      path.push(sub);
    }
    path
  }
  
  pub fn read_file(&self, path: PathBuf) -> String {
    let mut f = File::open(path).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    s
  }
}

