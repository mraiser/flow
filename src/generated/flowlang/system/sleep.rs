use ndata::dataobject::*;
use ndata::data::*;
use std::thread;
use std::time::Duration;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_i64("millis");
let ax = sleep(a0);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn sleep(mut millis:i64) -> String {
let dur = Duration::from_millis(millis as u64);
thread::sleep(dur);
"OK".to_string()
}

