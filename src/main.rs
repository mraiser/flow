use ::flowlang::*;

use std::env;
use std::io;
use std::io::BufRead;
use ndata::NDataConfig;
use ndata::dataobject::DataObject;

use command::Command as Command;
use datastore::DataStore;
use rustcmd::RustCmd;
use cmdinit::*;

pub fn init(dir:&str) -> (&str, NDataConfig) {
  let cfg = DataStore::init(dir);
  let mut v = Vec::new();
  cmdinit(&mut v);
  for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
  cfg
}

pub fn mirror(q:(&str, NDataConfig)) {
  DataStore::mirror(q);
  let mut v = Vec::new();
  cmdinit(&mut v);
  for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
}

pub fn main() {
  init("data");
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let params: Vec<String> = env::args().collect();
    let lib = &params[1];
    let ctl = &params[2];
    let cmd = &params[3];

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut s = "".to_string();
    while let Some(line) = lines.next() {
      s = s + &line.unwrap();
    }
    
    let args = DataObject::from_string(&s);
    let cmd = Command::lookup(lib, ctl, cmd);
    let res = cmd.execute(args).unwrap();
    println!("{}", res.to_string());
  }
}
