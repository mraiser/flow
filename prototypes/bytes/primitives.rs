use crate::dataobject::*;
use crate::dataproperty::*;
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub struct Primitive {
  pub name: String,
  pub inputs: DataObject,
  pub outputs: DataObject,
  pub func: fn(args:DataObject) -> DataObject,
}

impl Primitive {
  pub fn new(name: &str) -> Primitive {
    if name == "+" { return build_plus(); }
    if name == "-" { return build_minus(); }
    if name == "time" { return build_time(); }
    
    // FIXME - Should fail on unknown prim
    println!("Unknown primitive: {}", name);
    
    return Primitive { 
      name: name.to_string(),
      inputs: DataObject::new(),
      outputs: DataObject::new(),
      func: noop,
    };
  }
  
  pub fn execute(&self, args:DataObject) -> DataObject {
    (self.func)(args)
  }
}

fn noop(_args:DataObject) -> DataObject{
  DataObject::new()
}

fn time(_args:DataObject) -> DataObject {
  let mut o = DataObject::new();
  o.put_i64("a", std::time::SystemTime::now().duration_since(UNIX_EPOCH).expect("error").as_millis().try_into().unwrap());
  o
}

fn build_time() -> Primitive {
  let ins = DataObject::new();
  let mut outs = DataObject::new();
  outs.put_object("a", DataObject::new());
  Primitive {
    name: "time".to_string(),
    inputs: ins,
    outputs: outs,
    func: time,
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

fn build_plus() -> Primitive {
  let mut ins = DataObject::new();
  ins.put_object("a", DataObject::new());
  ins.put_object("b", DataObject::new());
  
  let mut outs = DataObject::new();
  outs.put_object("c", DataObject::new());

  Primitive {
    name: "+".to_string(),
    inputs: ins,
    outputs: outs,
    func: plus,
  }
}

fn minus(args:DataObject) -> DataObject{
  //println!("PRIM MINUS IN {:?}", &args);
  let a = args.get_property("a");
  let b = args.get_property("b");
  let mut out = DataObject::new();
  if a.is_number() && b.is_number() { // FIXME - Use match
    if a.is_f64() || b.is_f64() { 
      out.put_float("c", a.as_f64() - b.as_f64()); 
    }
    else {
      out.put_i64("c", a.as_i64() - b.as_i64()); 
    }
  }  
  else {
    out.put_str("c", "NaN");
  }
  //println!("PRIM MINUS OUT {:?}", &out);
  out
}

fn build_minus() -> Primitive {
  let mut ins = DataObject::new();
  ins.put_object("a", DataObject::new());
  ins.put_object("b", DataObject::new());
  
  let mut outs = DataObject::new();
  outs.put_object("c", DataObject::new());

  Primitive {
    name: "+".to_string(),
    inputs: ins,
    outputs: outs,
    func: minus,
  }
}

fn as_string(a:DataProperty) -> String {
  if a.is_f64() { return a.as_f64().to_string(); }
  if a.is_i64() { return a.as_i64().to_string(); }
  if a.is_string() { return a.as_string(); }
  "".to_string()
}
