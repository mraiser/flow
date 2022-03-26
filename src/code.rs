use serde::*;
use serde_json::*;
use std::collections::HashMap;
//use std::process::exit;

use super::primitives::Primitive;

#[derive(Debug, Serialize, Deserialize)]
pub struct Code {
  pub input: HashMap<String, Node>,
  pub output: HashMap<String, Node>,
  pub cmds: Vec<Command>,
  pub cons: Vec<Connection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
  #[serde(default)]
  pub mode: String,
  #[serde(default)]
  #[serde(rename = "type")]
  pub cmd_type: String,
  #[serde(default)]
  pub x:f32,
  #[serde(default)]
  pub val:Value,
  #[serde(default)]
  pub done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
  #[serde(rename = "in")]
  pub cmd_in: HashMap<String, Node>,
  pub out: HashMap<String, Node>,
  pub pos: Pos,
  pub name: String,
  pub width: f64,
  #[serde(rename = "type")]
  pub cmd_type: String,
  pub ctype: Option<String>,
  pub localdata: Option<Value>,
  
  #[serde(default)]
  pub done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Connection {
  pub src: Dest,
  pub dest: Dest,

  #[serde(default)]
  pub done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pos {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dest {
  index: i64,
  name: String,
}

impl Code {
  pub fn new(data: Value) -> Code {
    let s = data.to_string();
    let c: Code = serde_json::from_str(&s).unwrap();
    c
  }

  pub fn execute(mut self, args: Value) -> Value {
    let mut done = false;
    let mut out = json!({});
    
    let mut current_case = &mut self;
    
    while !done {
      let cmds = &mut current_case.cmds;
      let n2 = cmds.len();
      let cons = &mut current_case.cons;
      let n = cons.len();
      
      let mut i = 0;
      while i<n2{
        let mut cmd = &mut cmds[i];
        if !cmd.done {
          let mut count = 0;
          let mut b = true;
          for (key, mut value) in cmd.cmd_in.iter_mut() {
            count = count + 1;
            if let Some(_con) = lookup_con(cons, &key, "in"){
              b = false;
              break;
            }
            else {
              //println!("No input found!");
              value.done = true;
            }
          }
          if count == 0 || b {
            evaluate(&mut cmd)
          }
        }
        i = i + 1;
      }
      
      while !done {
        let mut c = true;
        let mut i = 0;
        while i<n {
          let mut con = &mut cons[i];
          if !con.done {
            c = false;
            let mut b = false;
            let mut val = json!(null);
            let src = con.src.index;
            let srcname = &con.src.name;
            let dest = con.dest.index;
            let destname = &con.dest.name;
            if src == -1 {
              if let Some(v) = args.get(srcname){
                val = v.to_owned();
                // FIXME - pretty sure this passes a copy-- should pass the instance.
              }
              b = true;
              //println!("FROM INPUTBAR {} {}", val, args);
            }
            else {
              let cmd = &mut cmds[src as usize];
              if cmd.done {
                val = cmd.out[srcname].val.to_owned();
                // FIXME - pretty sure this passes a copy-- should pass the instance.
                //println!("FROM CMD OUTPUT {}", val);
                b = true;
              }
            }
            
            if b {
              con.done = true;
              if dest == -2 {
                //println!("TO OUTPUTBAR {}", &val);
                out[destname] = val;
              }
              else {
                let mut cmd = &mut cmds[dest as usize];
                if cmd.cmd_type == "undefined" {
                  // FIXME - is this used?
                  println!("Marking undefined command as done");
                  cmd.done = true;
                }
                else {
                  let mut var = cmd.cmd_in.get_mut(destname).unwrap();
                  var.val = val;
                  var.done = true;
                  
                  for (_key, value) in cmd.cmd_in.iter_mut() {
                    b = b && value.done;
                    if !b { break; }
                  }
                  if b { evaluate(cmd); }
                }
              }
            }
          }
          i = i + 1;
        }
        if c {
          done = true;
        }
      }
    }
    // FIXME - Add NextCaseException and TerminateCaseException
    out
  }
}
  
fn lookup_con<'m>(cons: &'m Vec<Connection>, key: &str, which: &str) -> Option<&'m Connection> {
  let n = cons.len();
  let mut j = 0;
  while j<n{
    let con = &cons[j];
    let mut bar = &con.src;
    if which == "in" { bar = &con.dest }
    if bar.name == key {
//      println!("CON/KEY {:?} / {:?}", con, key);
      return Some(con);
    }
    j = j + 1;
  }
  
  None
}

fn evaluate<'m>(cmd: &'m mut Command) {
  let mut in1 = json!({});
  let in2 = &cmd.cmd_in;
  let mut list_in:Vec<String> = Vec::new();
  for (name, in3) in in2 {
    //println!("checking {:?}", in3);
    
    in1[name] = in3.val.to_owned();
    // FIXME - pretty sure this passes a copy-- should pass the instance.
    
    if in3.mode == "list" { list_in.push(name.to_owned()); }
  }
  
  let out2 = &cmd.out;
  let mut list_out:Vec<String> = Vec::new();    
  let mut loop_out:Vec<String> = Vec::new();    
  for (name, out3) in out2 {
    if out3.mode == "list" { list_out.push(name.to_owned()); }
    else if out3.mode == "loop" { loop_out.push(name.to_owned()); }    
  }
  
  let n = list_in.len();
  if n == 0 && loop_out.len() == 0 {
    evaluate_operation(cmd, in1);
  }
  else {
    // FIXME - implement lists & loops
    
    
    
    
    
    
    
    
  }
}

fn evaluate_operation(cmd:&mut Command, in1:Value) {
  let mut out = json!({});
  
  if cmd.cmd_type == "primitive" {
    let p = Primitive::new(&cmd.name);
    out = p.execute(in1);
  }
  else if cmd.cmd_type == "local" {
    let src = cmd.localdata.as_ref().unwrap().to_owned();
    let code = Code::new(src);
    out = code.execute(in1);
  }
  else if cmd.cmd_type == "constant" {
    for (key, _valmeta) in &cmd.out {
      let mut val = json!(null);
      let v = &cmd.name;
      let ctype = cmd.ctype.as_ref().unwrap();
      if ctype == "int" { val = json!(v.parse::<i64>().unwrap()); }
      else if ctype == "decimal" { val = json!(v.parse::<f64>().unwrap()); }
      else if ctype == "boolean" { val = json!(v.parse::<bool>().unwrap()); }
      else if ctype == "string" { val = json!(v); }
      else if ctype == "object" { val = serde_json::from_str(v).unwrap(); }
      else if ctype == "array" { val = serde_json::from_str(v).unwrap(); }
              
      //println!("key/val/ctype {} / {} / {}", key, val, ctype);
      out[key] = val;
    }
  }  
  else {
    println!("UNIMPLEMENTED {}", cmd.cmd_type);
  }
  
  
  
  
  
  
  
  
  
  
  
  
  
  // FIXME - Implement command, match
  
  
  // FIXME - Handle failexception & conditionals
  
  
  
  for (key, mut value) in cmd.out.iter_mut() {
    value.val = out[key].to_owned();
    // FIXME - pretty sure this passes a copy-- should pass the instance.
  }
  
  cmd.done = true;
}


