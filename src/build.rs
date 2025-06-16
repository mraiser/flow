use ::flowlang::*;

use std::env;

use datastore::*;
use buildrust::*;

fn main() {
  DataStore::init("data");

  env::set_var("RUST_BACKTRACE", "1");
  {
    let params: Vec<String> = env::args().collect();
    if params.len() < 2 || &params[1] == "ALL" {  // library names are lower case by convention
      build_all();
      rebuild_rust_api();
    }
    else if &params[1] == "API" {  // library names are lower case by convention
      rebuild_rust_api();
    }
    else {
      let store = DataStore::new();
      let root = store.get_lib_root(&params[1]);
      let ctl = &params[2];
      let cmd = &params[3];
      build(&params[1], ctl, cmd, &root);
    }
  }
}

