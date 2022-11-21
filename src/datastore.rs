use std::fs::File;
use std::path::*;
use std::io::Read;
use std::fs;
#[cfg(feature="serde_support")]
use serde_json::*;

use ndata::data::*;
use ndata::dataobject::*;
use ndata::dataarray::*;
use ndata::databytes::*;
use ndata::NDataConfig;

use crate::rustcmd::RustCmd;
use crate::rand::*;

static mut STORE_PATH:Option<PathBuf> = None;

#[derive(Debug)]
pub struct DataStore {
  pub root: PathBuf,
}

impl DataStore {
  pub fn init(dir:&str) -> (&str, NDataConfig) {
    let d = Path::new(dir);
    unsafe { STORE_PATH = Some(d.to_path_buf()); }
    
    Rand::init();
    let q = ndata::init();
    
    RustCmd::init();
    
    let o = DataObject::new();
    let _x = o.incr();
    (dir, q)
  }
  
  #[cfg(feature="mirror")]
  #[allow(dead_code)]
  pub fn mirror(q:(&str, NDataConfig)) {
    let d = Path::new(q.0);
    unsafe { STORE_PATH = Some(d.to_path_buf()); }
    
    Rand::init();
    ndata::mirror(q.1);
    
    RustCmd::init();
    
    let o = DataObject::new();
    let _x = o.incr();
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
  
  #[allow(dead_code)]
  pub fn globals() -> DataObject {
    DataObject::get(0)
  }
  
  #[allow(dead_code)]
  pub fn gc() {
    DataObject::gc();
    DataArray::gc();
    DataBytes::gc();
  }
  
  pub fn lib_info(&self, lib:&str) -> DataObject {
    let path = self.root.join(lib).join("meta.json");
    let s = self.read_file(path);
    DataObject::from_string(&s)
  }
  
  pub fn get_lib_root(&self, lib:&str) -> PathBuf {
    let meta = self.lib_info(&lib);

    let root;
    if meta.has("root") {
    let r = meta.get_string("root");
    if r.starts_with("/") {
      root = Path::new(&r).to_owned();
    }
    else {
      root = self.root.parent().unwrap().join(r).to_owned();
    }
    }
    else {
    root = self.root.parent().unwrap().join("cmd").to_owned();
    }
    
    root
  }
  
  pub fn lookup_cmd_id(&self, lib:&str, ctl:&str, cmd:&str) -> String {
    let data = self.get_data(lib, "controls");
    let data = data.get_object("data").get_array("list");
    for c in data.objects() {
      let c = c.object();
      let n = c.get_string("name");
      if n == ctl {
        let ctlid = c.get_string("id");
        let ctldata = self.get_data(lib, &ctlid);
        let ctldata = ctldata.get_object("data");
        for cmddata in ctldata.get_array("cmd").objects() {
          let cmddata = cmddata.object();
          let n2 = cmddata.get_string("name");
          if n2 == cmd {
            return cmddata.get_string("id");
          }
        }
      }
    }
    panic!("No such command {}:{}:{}", lib, ctl, cmd);
  }
  
  #[allow(dead_code)]
  pub fn set_data(&self, db: &str, id: &str, data:DataObject) {
    let mut d = data.get_object("data");
    if d.has("attachmentkeynames") {
      let v = d.get_array("attachmentkeynames");
      for b in v.objects() {
        let b = &b.string();
        let aid = id.to_string()+"."+b;
        let f = self.get_data_file(db, &aid);
        let s = Data::as_string(d.get_property(b));
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, s).expect("Unable to write file");
        d.remove_property(b);
      }
    }
  
    let s = data.to_string();
    let f = self.get_data_file(db, id);
    fs::create_dir_all(f.parent().unwrap()).unwrap();
    fs::write(f, s).expect("Unable to write file");
  }
  
  #[cfg(feature="serde_support")]
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
        if apath.exists(){
          let astr = self.read_file(apath);
          if astr.len() > 0 && astr[0..1].to_string() == "{" { // FIXME - Legacy hack
            data["data"][b] = serde_json::from_str(&astr).unwrap(); 
          } else {
            data["data"][b] = serde_json::Value::String(astr); 
          }
        }
      }
    }
    data
  }
  
  pub fn get_data(&self, db: &str, id: &str) -> DataObject {
    #[cfg(feature="serde_support")]
    {
      let data = self.get_json(db, id);
      return DataObject::from_json(data);
    }
    #[allow(unreachable_code)]
    {
      let path = self.get_data_file(db, id);
      let s = self.read_file(path);
      let data = DataObject::from_string(&s);
      let mut d = data.get_object("data");
      if d.has("attachmentkeynames") {
        let attachments: DataArray = d.get_array("attachmentkeynames");
        for a in attachments.objects(){
          let b = &a.string();
          let aid = id.to_string()+"."+b;
          let apath = self.get_data_file(db, &aid);
          let astr = self.read_file(apath);
          if astr.len() > 0 && astr[0..1].to_string() == "{" { // FIXME - Legacy hack
            d.put_object(b, DataObject::from_string(&astr)); 
          } else {
            d.put_string(b, &astr); 
          }
        }
      }
      return data;
    }
  }
  
  pub fn exists(&self, db: &str, id: &str) -> bool {
    self.get_data_file(db, id).exists()
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
    if !path.exists() { println!("Missing file {:?}", path); }
    let mut f = File::open(path).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    s
  }
}

