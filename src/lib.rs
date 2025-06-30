pub mod flowlang;

pub mod code;
pub mod case;
pub mod command;
pub mod datastore;
pub mod primitives;
pub mod rustcmd;
pub mod rand;
pub mod builder;
pub mod buildrust;
pub mod rfc2822date;
pub mod sha1;
pub mod base64;
pub mod appserver;
pub mod x25519;
pub mod mcp;

pub mod cmdinit;

#[cfg(feature="java_runtime")]
pub mod javacmd;
#[cfg(feature="javascript_runtime")]
pub mod jscmd;
#[cfg(feature="python_runtime")]
pub mod pyenv;
#[cfg(feature="python_runtime")]
pub mod pywrapper;
pub mod pycmd;

use datastore::DataStore;
use ndata::NDataConfig;
use crate::cmdinit::*;
use crate::rustcmd::RustCmd;
use ndata::dataobject::DataObject;

pub type Transform = fn(DataObject) -> DataObject;


pub fn init(dir:&str) -> (&str, NDataConfig) {
  //println!("flowlang::init()");
  let q = DataStore::init(dir);
  let mut v = Vec::new();
  cmdinit(&mut v);
  //for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }

  let mut cmd_map = DataObject::new();
  DataStore::globals().put_object("RUST_COMMANDS", cmd_map.clone());

  for q in &v {
    // Create a new object to hold the command's details.
    let cmd_details = RustCmd::detail(q.0.to_owned(), q.1, q.2.to_owned());
    // Put the details object into the main command map.
    //println!("INIT ADDING RUST COMMAND {}:{}", q.0, cmd_details.to_string());
    cmd_map.put_object(&q.0, cmd_details);
  }
  q
}

pub fn mirror(q:(&str, NDataConfig)) {
  //println!("flowlang::mirror()");
  DataStore::mirror(q);
  let mut v = Vec::new();
  cmdinit(&mut v);
  //for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
  let mut cmd_map = DataStore::globals().get_object("RUST_COMMANDS");

  for q in &v {
    // Create a new object to hold the command's details.
    let cmd_details = RustCmd::detail(q.0.to_owned(), q.1, q.2.to_owned());
    // Put the details object into the main command map.
    //println!("MIRROR ADDING RUST COMMAND {}:{}", q.0, cmd_details.to_string());
    cmd_map.put_object(&q.0, cmd_details);
  }
}

