use ndata::dataobject::*;
use std::time::UNIX_EPOCH;
use std::time::SystemTime;

pub fn execute(_o: DataObject) -> DataObject {
let ax = time();
let mut o = DataObject::new();
o.put_int("a", ax);
o
}

pub fn time() -> i64 {
SystemTime::now().duration_since(UNIX_EPOCH).expect("error").as_millis().try_into().unwrap()
}

