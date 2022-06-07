use std::env;
use std::io;
use std::io::BufRead;

mod code;
mod case;
mod command;
mod datastore;
mod primitives;
mod rustcmd;
mod generated;
mod rand;

use command::Command as Command;
use datastore::DataStore;
use ndata::dataobject::*;

fn main() {
  DataStore::init("data");
  
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
    
    let args = DataObject::from_json(serde_json::from_str(&s).unwrap());
    let cmd = Command::lookup(lib, ctl, cmd);
    let res = cmd.execute(args).unwrap();
    println!("{}", res.to_json());
    
//    DataObject::print_heap();
//    DataArray::print_heap();
//    DataBytes::print_heap();
  }
//  DataStore::gc();
//  DataObject::print_heap();
//  DataArray::print_heap();
//  DataBytes::print_heap();
}
