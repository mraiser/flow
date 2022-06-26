use ndata::dataobject::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = stdout(a0);
let mut o = DataObject::new();
o.put_i64("a", ax);
o
}

pub fn stdout(mut a:Data) -> i64 {
println!("{}",Data::as_string(a));
1
}

