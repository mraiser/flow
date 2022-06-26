use std::env;
use std::io;
use std::io::BufRead;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::*;
use serde_json::Value;
use serde_json::json;
use std::fs::create_dir_all;
use std::fs::OpenOptions;

mod datastore;
use datastore::*;

mod buildrust;
use buildrust::*;

mod rand;

fn main() {
  DataStore::init("data");

  env::set_var("RUST_BACKTRACE", "1");
  {
    let params: Vec<String> = env::args().collect();
    let lib = &params[1];
    if lib == "all" {
      buildAll();
    }
    else {
      let ctl = &params[2];
      let cmd = &params[3];
      build(lib, ctl, cmd);
    }
  }
}

