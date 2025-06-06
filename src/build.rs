use ::flowlang::*;

use std::env;

use datastore::*;
use buildrust::*;

fn main() {
  DataStore::init("data");

  env::set_var("RUST_BACKTRACE", "1");
  {
    let params: Vec<String> = env::args().collect();
    let lib = &params[1];
    if lib == "ALL" {  // library names are lower case by convention
      build_all();
    }
    if lib == "API" {  // library names are lower case by convention
      rebuild_rust_api();
    }
    else {
      let store = DataStore::new();
      let root = store.get_lib_root(&lib);
      let ctl = &params[2];
      let cmd = &params[3];
      build(lib, ctl, cmd, &root);
    }
  }
}

