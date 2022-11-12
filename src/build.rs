use std::env;

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
      build_all();
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

