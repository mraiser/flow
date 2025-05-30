pub mod flowlang;

pub mod code;
pub mod case;
pub mod command;
pub mod datastore;
pub mod primitives;
pub mod rustcmd;
pub mod rand;
pub mod buildrust;
pub mod rfc2822date;
pub mod sha1;
pub mod base64;
pub mod appserver;
pub mod pycmd;
pub mod x25519;
pub mod mcp;

#[cfg(feature="java_runtime")]
pub mod javacmd;
#[cfg(feature="javascript_runtime")]
pub mod jscmd;
#[cfg(feature="python_runtime")]
pub mod pyenv;

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

