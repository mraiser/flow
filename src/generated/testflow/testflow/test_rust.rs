use ndata::dataobject::*;

pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("a");
let ax = test_rust(a0);
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn test_rust(a:String) -> String {
"Hello ".to_string()+&a+" from rust"
}

