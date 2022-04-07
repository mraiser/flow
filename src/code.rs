use crate::primitives::Primitive;
use crate::dataobject::*;
use crate::dataarray::*;
use crate::bytesref::*;
use crate::dataproperty::*;

#[derive(Debug)]
pub struct Code {
  pub data: DataObject,
}

impl Code {
  pub fn new(data: DataObject) -> Code {
    Code {
      data: data,
    }
  }

  pub fn execute(self, args: DataObject) -> DataObject {
    let mut done = false;
    let mut out = DataObject::new();
    
    let current_case = self.data;
    
    while !done {
      let cmds = current_case.get_array("cmds");
      let n2 = cmds.len();
      let cons = current_case.get_array("cons");
      let n = cons.len();
      
      let mut i = 0;
      while i<n2{
        let mut cmd = cmds.get_object(i);
        if !cmd.has("done") { cmd.put_bool("done", false); }
        if !cmd.get_bool("done") {
          let mut count = 0;
          let mut b = true;
          for dp in &cmd.get_object("in") {
            let key = cmd.lookup_prop_string(dp.id);
            count = count + 1;
            if let Some(_con) = lookup_con(&cons, &key, "in"){
              b = false;
              break;
            }
            else {
              //println!("No input found!");
              cmd.get_object(&key).put_bool("done", true);
            }
          }
          if count == 0 || b {
            evaluate(cmd)
          }
        }
        i = i + 1;
      }
      
      while !done {
        let mut c = true;
        let mut i = 0;
        while i<n {
          let mut con = cons.get_object(i);
          if !con.has("done") { con.put_bool("done", false); }
          if !con.get_bool("done") {
            c = false;
            let mut b = false;
            let mut val = DataProperty::new(0, TYPE_NULL, BytesRef::push(Vec::<u8>::new()));
            let ja = con.get_array("src");
            let src = ja.get_i64(0);
            let srcname = ja.get_string(1);
            let ja = con.get_array("dest");
            let dest = ja.get_i64(0);
            let destname = ja.get_string(1);
            if src == -1 {
              if args.has(&srcname){
                val = args.get_property(&srcname);
              }
              b = true;
            }
            else {
              let cmd = cmds.get_object(src as usize);
              if cmd.get_bool("done") {
                val = cmd.get_object("out").get_object(&srcname).get_property("val");
                b = true;
              }
            }
            
            if b {
              let newbr = BytesRef::get(val.byte_ref, val.off, val.len);
              con.put_bool("done", true);
              if dest == -2 {
                out.set_property(&destname, val.typ, newbr);
              }
              else {
                let mut cmd = cmds.get_object(dest as usize);
                if cmd.get_string("type") == "undefined" {
                  // FIXME - is this used?
                  println!("Marking undefined command as done");
                  cmd.put_bool("done", true);
                }
                else {
                  let mut var = cmd.get_object("in").get_object(&destname);
                  var.set_property("val", val.typ, newbr);
                  var.put_bool("done", true);
                  
                  let input = cmd.get_object("in");
                  for dp in &input {
                    let key = cmd.lookup_prop_string(dp.id);
                    let mut value = input.get_object(&key);
                    if !value.has("done") { value.put_bool("done", false); }
                    b = b && value.get_bool("done");
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
  
fn lookup_con<'m>(cons: &DataArray, key: &str, which: &str) -> Option<DataObject> {
  //println!("Looking into {:?}", cons); 
  let n = cons.len();
  let mut j = 0;
  while j<n{
    let con = cons.get_object(j);
    //println!("Looking at {:?}", con); 
    let mut bar = con.get_array("src");
    if which == "in" { bar = con.get_array("dest") } // FIXME - Use match instead
    //println!("Looking up '{}'/'{}' {:?}", key, which, bar);
    if bar.get_string(1) == key {
//      println!("CON/KEY {:?} / {:?}", con, key);
      return Some(con);
    }
    j = j + 1;
  }
  
  None
}

fn evaluate(cmd: DataObject) {
  let mut in1 = DataObject::new();
  let in2 = cmd.get_object("in");
  let mut list_in:Vec<String> = Vec::new();
  for dp in &in2 {
    let name = cmd.lookup_prop_string(dp.id);
    let in3 = in2.get_object(&name);
    //println!("checking {:?}", in3);
    
    let dp3 = in3.get_property("val");
    let br3 = BytesRef::get(dp3.byte_ref, dp3.off, dp3.len);
    in1.set_property(&name, dp3.typ, br3);
    
    if in3.has("mode") && in3.get_string("mode") == "list" { list_in.push(name); }
  }
  
  let out2 = cmd.get_object("out");
  let mut list_out:Vec<String> = Vec::new();    
  let mut loop_out:Vec<String> = Vec::new();    
  for dp in &out2 {
    let name = cmd.lookup_prop_string(dp.id);
    let out3 = out2.get_object(&name);
    if out3.has("mode") {
      let mode = out3.get_string("mode");
      if mode == "list" { list_out.push(name); }
      else if mode == "loop" { loop_out.push(name); }    
    }
  }
  
  let n = list_in.len();
  if n == 0 && loop_out.len() == 0 {
    evaluate_operation(cmd, in1);
  }
  else {
    // FIXME - implement lists & loops
    
    
    
    
    
    
    
    
  }
}

fn evaluate_operation(mut cmd:DataObject, in1:DataObject) {
  let mut out = DataObject::new();
  let cmd_type = cmd.get_string("type");
  let v = &cmd.get_string("name");
  if cmd_type == "primitive" { // FIXME - use match
    let p = Primitive::new(v);
    out = p.execute(in1);
  }
  else if cmd_type == "local" {
    let src = cmd.get_object("localdata");
    let code = Code::new(src);
    out = code.execute(in1);
  }
  else if cmd_type == "constant" {
    for dp in &cmd.get_object("out") {
      let key = &cmd.lookup_prop_string(dp.id);
      let ctype = cmd.get_string("ctype");
      if ctype == "int" { out.put_i64(key, v.parse::<i64>().unwrap()); }
      else if ctype == "decimal" { out.put_float(key, v.parse::<f64>().unwrap()); }
      else if ctype == "boolean" { out.put_bool(key, v.parse::<bool>().unwrap()); }
      else if ctype == "string" { out.put_str(key, v); }
      else if ctype == "object" { 
        out.put_object(key, DataObject::from_json(serde_json::from_str(v).unwrap())); 
      }
      else if ctype == "array" { 
        out.put_list(key, DataArray::from_json(serde_json::from_str(v).unwrap())); 
      }
      else { out.put_null(v); }
    }
  }  
  else {
    println!("UNIMPLEMENTED {}", cmd_type);
  }
  
  
  
  
  
  
  
  
  
  
  
  
  // FIXME - Implement command, match
  
  
  // FIXME - Handle failexception & conditionals
  
  
  let cmd_out = cmd.get_object("out");
  for dp in &cmd_out {
    let key = &cmd.lookup_prop_string(dp.id);
    let mut value = cmd_out.get_object(key);
    let newdp = out.get_property(key);
    let newbr = BytesRef::get(newdp.byte_ref, newdp.off, newdp.len);
    value.set_property("val", newdp.typ, newbr);
  }
  
  cmd.put_bool("done", true);
}


