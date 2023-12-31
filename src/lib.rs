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

mod cmdinit;

#[cfg(feature="java_runtime")]
pub mod javacmd;
#[cfg(feature="javascript_runtime")]
pub mod jscmd;
#[cfg(feature="python_runtime")]
pub mod pyenv;
pub mod pycmd;

use datastore::DataStore;
use ndata::NDataConfig;
use crate::cmdinit::*;
use crate::rustcmd::RustCmd;

pub fn init(dir:&str) -> (&str, NDataConfig) {
  let q = DataStore::init(dir);
  let mut v = Vec::new();
  cmdinit(&mut v);
  for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
  q
}

pub fn mirror(q:(&str, NDataConfig)) {
  DataStore::mirror(q);
  let mut v = Vec::new();
  cmdinit(&mut v);
  for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
}

