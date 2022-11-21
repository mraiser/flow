use ndata::dataobject::*;
use crate::datastore::*;

use crate::flowlang::system::random_non_hex_char::random_non_hex_char;
use crate::flowlang::system::time::time;

pub fn execute(_o: DataObject) -> DataObject {
let ax = unique_session_id();
let mut o = DataObject::new();
o.put_string("a", &ax);
o
}

pub fn unique_session_id() -> String {
let mut globals = DataStore::globals();
if !globals.has("last_session_index") { globals.put_int("last_session_index", 0); }
let last_id = globals.get_int("last_session_index");
let mut next_id = last_id + 1;
if next_id > 65535 { next_id = 0; }
globals.put_int("last_session_index", next_id);

let s = random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &random_non_hex_char()
  + &format!("{:x}", time())
  + &random_non_hex_char()
  + &format!("{:x}", last_id);

s
}

