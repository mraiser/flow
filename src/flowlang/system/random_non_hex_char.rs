use ndata::dataobject::*;
use crate::rand::*;

pub fn execute(_o: DataObject) -> DataObject {
let ax = random_non_hex_char();
let mut o = DataObject::new();
o.put_str("a", &ax);
o
}

pub fn random_non_hex_char() -> String {
let nonhexchars = "ghijklmnopqrstuvwxyz";
let x = rand_range(0,19) as usize;
nonhexchars[x..x+1].to_string()
}

