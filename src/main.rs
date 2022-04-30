use std::path::Path;
use std::env;
use std::io;
use std::io::BufRead;

mod code;
mod case;
mod command;
mod datastore;
mod primitives;
mod data;
mod heap;
mod dataobject;
mod dataarray;
mod flowenv;
mod usizemap;

use command::Command as Command;
use datastore::DataStore;
use dataobject::*;
use flowenv::*;

fn main() {
  let path = Path::new("data");
  let store = DataStore::new(path.to_path_buf());
  
  FlowEnv::init(store);
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let params: Vec<String> = env::args().collect();
    let lib = &params[1];
    let id = &params[2];

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut s = "".to_string();
    while let Some(line) = lines.next() {
      s = s + &line.unwrap();
    }
    
    let args = DataObject::from_json(serde_json::from_str(&s).unwrap());
    let cmd = Command::new(lib, id);
    let res = cmd.execute(args).unwrap();
    
    println!("{}", res.to_json());
  }
}
