use crate::dataobject::*;
use crate::dataproperty::*;

#[derive(Debug)]
pub struct Primitive {
  pub name: String,
  pub inputs: DataObject,
  pub outputs: DataObject,
  pub func: fn(args:DataObject) -> DataObject,
}

impl Primitive {
  pub fn new(name: &str) -> Primitive {
    // FIXME - Hard-coded for Plus. Return primitive based on name, with inputs & outputs
    return Primitive { 
      name: name.to_string(),
      inputs: DataObject::new(),
      outputs: DataObject::new(),
      func: plus,
    };
  }
  
  pub fn execute(&self, args:DataObject) -> DataObject {
    (self.func)(args)
  }
}

fn plus(args:DataObject) -> DataObject{
  //println!("PRIM PLUS IN {:?}", &args);
  let a = args.get_property("a");
  let b = args.get_property("b");
  let mut out = DataObject::new();
  if a.is_number() && b.is_number() { // FIXME - Use match
    if a.is_f64() || b.is_f64() { 
      out.put_float("c", a.as_f64() + b.as_f64()); 
    }
    else {
      out.put_i64("c", a.as_i64() + b.as_i64()); 
    }
  }  
  else {
    out.put_str("c", &(as_string(a)+&as_string(b)));
  }
  //println!("PRIM PLUS OUT {:?}", &out);
  out
}

fn as_string(a:DataProperty) -> String {
  if a.is_f64() { return a.as_f64().to_string(); }
  if a.is_i64() { return a.as_i64().to_string(); }
  if a.is_string() { return a.as_string(); }
  "".to_string()
}
