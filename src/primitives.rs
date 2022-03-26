use serde_json::*;

#[derive(Debug)]
pub struct Primitive {
  pub name: String,
  pub inputs: Value,
  pub outputs: Value,
  pub func: fn(args:Value) -> Value,
}

impl Primitive {
  pub fn new(name: &str) -> Primitive {
    return Primitive {
      name: name.to_string(),
      inputs: json!("{}"),
      outputs: json!("{}"),
      func: plus,
    };
  }
  
  pub fn execute(&self, args:Value) -> Value {
    (self.func)(args)
  }
}

fn plus(args:Value) -> Value{
  let a = args["a"].to_owned();
  let b = args["b"].to_owned();
  let c;
  //println!("PLUS {} {} {}", args, a, b);
  if a.is_number() && b.is_number() {
    if a.is_f64() || b.is_f64() { c = json!(a.as_f64().unwrap() + b.as_f64().unwrap()); }
    else { c = json!(a.as_i64().unwrap() + b.as_i64().unwrap()); }
  }  
  else {
    c = json!(as_string(a)+&as_string(b));
  }
  
  let mut out = json!({});
  out["c"] = c;
  out
}

fn as_string(a:Value) -> String {
  if a.is_f64() { return a.as_f64().unwrap().to_string(); }
  if a.is_i64() { return a.as_i64().unwrap().to_string(); }
  if a.is_string() { return a.as_str().unwrap().to_string(); }
  "".to_string()
}
