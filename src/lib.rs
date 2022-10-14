pub mod code;
pub mod case;
pub mod command;
pub mod datastore;
pub mod primitives;
pub mod rustcmd;
pub mod generated;
pub mod rand;
pub mod buildrust;
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

use datastore::DataStore;
use generated::Generated;
use ndata::NDataConfig;

pub fn init(dir:&str) -> (&str, NDataConfig) {
  let q = DataStore::init(dir);
  Generated::init();
  q
}

#[cfg(feature="reload")]
pub fn mirror(q:(&str, NDataConfig)) {
  DataStore::mirror(q);
  Generated::init();
}

