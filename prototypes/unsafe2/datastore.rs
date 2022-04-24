use serde_json::*;
use std::fs::File;
use std::path::*;
use std::io::Read;

//use crate::dataobject::*;

#[derive(Debug)]
pub struct DataStore {
  pub root: PathBuf,
}

impl DataStore {
  pub fn new(root: PathBuf) -> DataStore {
    return DataStore {
      root: root,
    };
  }
  
  pub fn clone(&self) -> DataStore {
    return DataStore {
      root: self.root.to_owned(),
    };
  }
  
  pub fn get_data(&self, db: &str, id: &str) -> Value { //DataObject {
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
        if astr[0..1].to_string() == "{" { // FIXME - Legacy hack
          data["data"][b] = serde_json::from_str(&astr).unwrap(); 
        } else {
          data["data"][b] = serde_json::Value::String(astr); 
        }
      }
    }
//    DataObject::from_json(data)
    data
  }
  
  fn get_data_file(&self, db: &str, id: &str) -> PathBuf {
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
      let sub = &id[n..m];
      path.push(sub);
    }
    path
  }
  
  fn read_file(&self, path: PathBuf) -> String {
    let mut f = File::open(path).unwrap();
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    s
  }
}

#[test]
fn verify_test() {
  let path = Path::new("data");
  let store = DataStore::new(path.to_path_buf());
  let codeval = store.get_data("testflow", "zkuwhn1802d57cb8ak1c");
  let id = &codeval["id"];
  assert_eq!("zkuwhn1802d57cb8ak1c", id);
}

