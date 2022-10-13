pub mod code;
pub mod case;
pub mod command;
pub mod datastore;
pub mod primitives;
pub mod rustcmd;
pub mod generated;
pub mod rand;
pub mod rfc2822date;
pub mod sha1;
pub mod base64;
pub mod appserver;
#[cfg(feature="java_runtime")]
pub mod javacmd;
#[cfg(feature="javascript_runtime")]
pub mod jscmd;
#[cfg(feature="python_runtime")]
pub mod pycmd;

use std::env;
use std::io;
use std::io::BufRead;
use ndata::dataobject::*;

use command::Command as Command;
use datastore::DataStore;
use generated::Generated;

pub fn init(dir:&str) -> (&str, (((u64,u64),(u64,u64)),((u64,u64),(u64,u64)),((u64,u64),(u64,u64)))) {
  Generated::init();
  DataStore::init(dir)
}

pub fn mirror(q:(&str, (((u64,u64),(u64,u64)),((u64,u64),(u64,u64)),((u64,u64),(u64,u64))))) {
  Generated::init();
  DataStore::mirror(q)
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
