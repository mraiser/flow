use crate::dataobject::*;
use crate::data::*;
use crate::flowenv::*;
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub struct Primitive<'a> {
  pub name: String,
  pub inputs: DataObject<'a>,
  pub outputs: DataObject<'a>,
  pub func: fn(args:DataObject<'a>) -> DataObject<'a>,
}

impl<'a> Primitive<'a> {
  pub fn new(name: &str, env:&'a FlowEnv) -> Primitive<'a> {
    if name == "+" { return build_plus(env); }
    if name == "-" { return build_minus(env); }
    if name == "time" { return build_time(env); }
    
    // FIXME - Should fail on unknown prim
    println!("Unknown primitive: {}", name);
    
    return Primitive { 
      name: name.to_string(),
      inputs: DataObject::new(env),
      outputs: DataObject::new(env),
      func: noop,
    };
  }
  
  pub fn execute(&self, args:DataObject<'a>) -> DataObject<'a> {
    (self.func)(args)
  }
}

fn noop(args:DataObject) -> DataObject{
  DataObject::new(args.env)
}

fn time(args:DataObject) -> DataObject {
  let mut o = DataObject::new(args.env);
  o.put_i64("a", std::time::SystemTime::now().duration_since(UNIX_EPOCH).expect("error").as_millis().try_into().unwrap());
  o
}

fn build_time<'a>(env:&'a FlowEnv) -> Primitive<'a> {
  let ins = DataObject::new(env);
  let mut outs = DataObject::new(env);
  outs.put_object("a", DataObject::new(env));
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
  let mut out = DataObject::new(args.env);
  if a.is_number() && b.is_number() {
    if a.is_float() || b.is_float() { 
      out.put_float("c", a.float() + b.float()); 
    }
    else {
      out.put_i64("c", a.int() + b.int()); 
    }
  }  
  else {
    out.put_str("c", &(as_string(a)+&as_string(b)));
  }
  //println!("PRIM PLUS OUT {:?}", &out);
  out
}

fn build_plus<'a>(env:&'a FlowEnv) -> Primitive<'a> {
  let mut ins = DataObject::new(env);
  ins.put_object("a", DataObject::new(env));
  ins.put_object("b", DataObject::new(env));
  
  let mut outs = DataObject::new(env);
  outs.put_object("c", DataObject::new(env));

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
  let mut out = DataObject::new(args.env);
  if a.is_number() && b.is_number() {
    if a.is_float() || b.is_float() { 
      out.put_float("c", a.float() - b.float()); 
    }
    else {
      out.put_i64("c", a.int() - b.int()); 
    }
  }  
  else {
    out.put_str("c", "NaN");
  }
  //println!("PRIM MINUS OUT {:?}", &out);
  out
}

fn build_minus<'a>(env:&'a FlowEnv) -> Primitive<'a> {
  let mut ins = DataObject::new(env);
  ins.put_object("a", DataObject::new(env));
  ins.put_object("b", DataObject::new(env));
  
  let mut outs = DataObject::new(env);
  outs.put_object("c", DataObject::new(env));

  Primitive {
    name: "+".to_string(),
    inputs: ins,
    outputs: outs,
    func: minus,
  }
}

fn as_string(a:Data) -> String {
  if a.is_float() { return a.float().to_string(); }
  if a.is_int() { return a.int().to_string(); }
  if a.is_string() { return a.string(); }
  "".to_string()
}
