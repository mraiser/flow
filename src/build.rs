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
    else if &params[1] == "BUNDLES" {
      // The esbuild bundle of every JavaScript/TypeScript command that runs
      // from one (imports, or TypeScript): what a deployment ships so that it
      // needs no esbuild or node_modules.
      let (done, failed) = builder::javascript::build_bundles(&DataStore::new());
      for label in &done { println!("bundled {}", label); }
      for (label, why) in &failed { eprintln!("FAILED {}: {}", label, why); }
      println!("{} bundled, {} failed", done.len(), failed.len());
      if !failed.is_empty() { std::process::exit(1); }
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

