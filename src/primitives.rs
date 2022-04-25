use crate::dataobject::*;
use crate::data::*;
use std::time::UNIX_EPOCH;
use std::time::SystemTime;
use crate::flowenv::*;

pub struct Primitive {
  pub name: String,
//  pub inputs: DataObject,
//  pub outputs: DataObject,
  pub func: fn(args:DataObject, env:&mut FlowEnv) -> DataObject,
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
//      inputs: DataObject::new(env),
//      outputs: DataObject::new(env),
      func: noop,
    };
  }
  
  pub fn execute(&self, args:DataObject, env:&mut FlowEnv) -> DataObject {
    (self.func)(args, env)
  }
}

fn noop(_args:DataObject, env:&mut FlowEnv) -> DataObject{
  DataObject::new(env)
}

pub fn current_time_millis() -> i64 {
  SystemTime::now().duration_since(UNIX_EPOCH).expect("error").as_millis().try_into().unwrap()
}

fn time(_args:DataObject, env:&mut FlowEnv) -> DataObject {
  let mut o = DataObject::new(env);
  o.put_i64("a", current_time_millis(), env);
  o
}

fn build_time() -> Primitive {
//  let ins = DataObject::new(env);
//  let mut outs = DataObject::new(env);
//  outs.put_object("a", DataObject::new(env), env);
  Primitive {
    name: "time".to_string(),
//    inputs: ins,
//    outputs: outs,
    func: time,
  }
}

fn plus(args:DataObject, env:&mut FlowEnv) -> DataObject{
  //println!("PRIM PLUS IN {:?}", &args);
  let a = args.get_property("a", env);
  let b = args.get_property("b", env);
  let mut out = DataObject::new(env);
  if a.is_number() && b.is_number() {
    if a.is_float() || b.is_float() { 
      out.put_float("c", a.float() + b.float(), env); 
    }
    else {
      out.put_i64("c", a.int() + b.int(), env); 
    }
  }  
  else {
    out.put_str("c", &(as_string(a)+&as_string(b)), env);
  }
  //println!("PRIM PLUS OUT {:?}", &out);
  out
}

fn build_plus() -> Primitive {
//  let mut ins = DataObject::new(env);
//  ins.put_object("a", DataObject::new(env), env);
//  ins.put_object("b", DataObject::new(env), env);
  
//  let mut outs = DataObject::new(env);
//  outs.put_object("c", DataObject::new(env), env);

  Primitive {
    name: "+".to_string(),
//    inputs: ins,
//    outputs: outs,
    func: plus,
  }
}

fn minus(args:DataObject, env:&mut FlowEnv) -> DataObject{
  //println!("PRIM MINUS IN {:?}", &args);
  let a = args.get_property("a", env);
  let b = args.get_property("b", env);
  let mut out = DataObject::new(env);
  if a.is_number() && b.is_number() {
    if a.is_float() || b.is_float() { 
      out.put_float("c", a.float() - b.float(), env); 
    }
    else {
      out.put_i64("c", a.int() - b.int(), env); 
    }
  }  
  else {
    out.put_str("c", "NaN", env);
  }
  //println!("PRIM MINUS OUT {:?}", &out);
  out
}

fn build_minus() -> Primitive {
//  let mut ins = DataObject::new(env);
//  ins.put_object("a", DataObject::new(env), env);
//  ins.put_object("b", DataObject::new(env), env);
  
//  let mut outs = DataObject::new(env);
//  outs.put_object("c", DataObject::new(env), env);

  Primitive {
    name: "+".to_string(),
//    inputs: ins,
//    outputs: outs,
    func: minus,
  }
}

fn as_string(a:Data) -> String {
  if a.is_float() { return a.float().to_string(); }
  if a.is_int() { return a.int().to_string(); }
  if a.is_string() { return a.string(); }
  "".to_string()
}
