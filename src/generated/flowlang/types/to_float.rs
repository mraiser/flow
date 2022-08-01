use ndata::dataobject::*;
use ndata::data::*;
use ndata::data::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_property("a");
let ax = to_float(a0);
let mut o = DataObject::new();
o.put_float("a", ax);
o
}

pub fn to_float(mut a:Data) -> f64 {
let s = Data::as_string(a);
s.parse::<f64>().unwrap()
}

