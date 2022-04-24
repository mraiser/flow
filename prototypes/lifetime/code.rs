use std::cmp;

use crate::primitives::Primitive;
use crate::dataobject::*;
use crate::dataarray::*;
use crate::data::*;
//use crate::datastore::DataStore;
use crate::flowenv::FlowEnv;
use crate::command::Command;

#[derive(PartialEq, Debug)]
pub enum CodeException {
    Fail,
    Terminate,
    NextCase,
}

#[derive(Debug)]
pub struct Code<'a> {
  pub data: DataObject<'a>,
  pub env: &'a FlowEnv,
  pub finishflag: bool,
}

impl<'a> Code<'a> {
  pub fn new(data: DataObject<'a>, env: &'a FlowEnv) -> Code<'a> {
    Code {
      data: data,
      env: env,
      finishflag: false,
    }
  }

  pub fn execute(&'a mut self, args: DataObject<'a>) -> Result<DataObject<'a>, CodeException> {
    let mut done = false;
    let mut out = DataObject::new(self.env);
    
    let mut current_case = self.data.duplicate();
    
    while !done {
      let res = self.execute_loop(&mut current_case, &args, &mut out);
      if let Err(e) = res {
        if e == CodeException::NextCase {
          current_case = current_case.get_object("nextcase");
        }
        else if e == CodeException::Terminate {
          break;
        }
        else {
          return Err(e);
        }
      }
      else {
        done = res.unwrap();
      }
    }
    
    Ok(out)
  }
  
  fn execute_loop(&'a self, current_case: &'a mut DataObject, args: &'a DataObject, out: &'a mut DataObject) -> Result<bool, CodeException> {
    let cmds = current_case.get_array("cmds");
    let n2 = cmds.len();
    let cons = current_case.get_array("cons");
    let n = cons.len();
    let mut done = false;
    
    let mut i = 0;
    while i<n2{
      let mut cmd = cmds.get_object(i);
      if !cmd.has("done") { cmd.put_bool("done", false); }
      if !cmd.get_bool("done") {
        let mut count = 0;
        let mut b = true;
        for (key,_value) in cmd.get_object("in") {
          count = count + 1;
          if let Some(_con) = self.lookup_con(&cons, &key, "in"){
            b = false;
            break;
          }
          else {
            //println!("No input found!");
            cmd.get_object(&key).put_bool("done", true);
          }
        }
        if count == 0 || b {
          self.evaluate(cmd)?;
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
          let mut val = Data::DNull;
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
              //println!("SRCNAME {}", &srcname);
              val = cmd.get_object("out").get_property(&srcname);
              b = true;
            }
          }
          
          if b {
            con.put_bool("done", true);
            if dest == -2 {
              out.set_property(&destname, val);
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
                var.set_property("val", val);
                var.put_bool("done", true);
                
                let input = cmd.get_object("in");
                for (_key,v) in input {
                  let mut value = v.object(self.env);
                  if !value.has("done") { value.put_bool("done", false); }
                  b = b && value.get_bool("done");
                  if !b { break; }
                }
                if b { self.evaluate(cmd)?; }
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
    Ok(done)
  }
    
  fn lookup_con<'m>(&self, cons: &'a DataArray, key: &str, which: &str) -> Option<DataObject<'a>> {
    let n = cons.len();
    let mut j = 0;
    while j<n{
      let con = cons.get_object(j);
      let mut bar = con.get_array("src");
      if which == "in" { bar = con.get_array("dest") } // FIXME - Use match instead
      if bar.get_string(1) == key {
        return Some(con);
      }
      j = j + 1;
    }
    
    None
  }

  fn evaluate(&'a self, mut cmd: DataObject<'a>) -> Result<DataObject<'a>, CodeException> {
    let mut in1 = DataObject::new(self.env);
    let in2 = cmd.get_object("in");
    let mut list_in:Vec<String> = Vec::new();
    for (name,v) in in2 {
      let in3 = v.object(self.env);
      
      let dp3 = in3.get_property("val");
      in1.set_property(&name, dp3);
      
      if in3.has("mode") && in3.get_string("mode") == "list" { list_in.push(name); }
    }
    
    let out2 = cmd.get_object("out");
    let mut list_out:Vec<String> = Vec::new();    
    let mut loop_out:Vec<String> = Vec::new();    
    for (name,v) in out2.duplicate() {
      let out3 = v.object(self.env);
      if out3.has("mode") {
        let mode = out3.get_string("mode");
        if mode == "list" { list_out.push(name); }
        else if mode == "loop" { loop_out.push(name); }    
      }
    }
    
    let n = list_in.len();
    if n == 0 && loop_out.len() == 0 {
      return self.evaluate_operation(cmd, in1);
    }
    else {
      let mut out3 = DataObject::new(self.env);
      for key in &list_out { out3.put_list(&key, DataArray::new(self.env)); }
      let mut count = 0;
      if n>0 {
        count = in1.get_array(&list_in[0]).len();
        let mut i = 1;
        while i<n {
          count = cmp::min(count, in1.get_array(&list_in[i]).len());
          i = i + 1;
        }
      }
      
      let mut i = 0;
      loop {
        let mut in3 = DataObject::new(self.env);
        let list = in1.duplicate().keys();
        for key in list {
          if !list_in.contains(&key) { 
            let dp = in1.get_property(&key);
            in3.set_property(&key, dp); 
          }
          else {
            let ja = in1.get_array(&key);
            let dp = ja.get_property(i);
            in3.set_property(&key, dp); 
          }
        }

        self.evaluate_operation(cmd.duplicate(), in3)?;
        
        let out = cmd.get_object("out");
        for (k,_v) in out2.duplicate() {
          if out.has(&k) {
            let dp = out.get_property(&k);
            if list_out.contains(&k) {
              out3.get_array(&k).push_property(dp);
            }
            else {
              out3.set_property(&k, dp.clone());
              if loop_out.contains(&k) {
                let newk = out2.get_object(&k).get_string("loop");
                in1.set_property(&newk, dp.clone());
              }
            }
          }
        }
        
        if cmd.has("FINISHED") && cmd.get_bool("FINISHED") {
          break;
        }
        
        if n>0 {
          i = i + 1;
          if i == count {
            break;
          }
        }
      }
      
      cmd.put_object("out", out3.duplicate());
      return Ok(out3);
    }
  }

  fn evaluate_operation(&'a self, mut cmd:DataObject<'a>, in1:DataObject<'a>) -> Result<DataObject<'a>, CodeException> {
    let mut out = DataObject::<'a>::new(self.env); // FIXME - Don't instantiate here, leave unassigned
    let cmd_type = cmd.get_string("type");
    let mut b = true;
    let v = &cmd.get_string("name");
    
    let evaluation: Result<(), CodeException> = (|| {
      if cmd_type == "primitive" { // FIXME - use match
        let p = Primitive::new(v, self.env);
        out = p.execute(in1);
      }
      else if cmd_type == "local" {
        let src:DataObject<'a> = cmd.get_object("localdata");
        let src2 = src.deep_copy();
        let dup = DataObject::get(src2.data_ref, self.env);
        let mut code = Code::new(dup, self.env);
        let xout = code.execute(in1)?;
        out = DataObject::get(xout.data_ref, self.env);
        cmd.put_bool("FINISHED", code.finishflag);
      }
      else if cmd_type == "constant" {
        for (key,_x) in cmd.get_object("out") {
          let ctype = cmd.get_string("ctype");
          if ctype == "int" { out.put_i64(&key, v.parse::<i64>().unwrap()); }
          else if ctype == "decimal" { out.put_float(&key, v.parse::<f64>().unwrap()); }
          else if ctype == "boolean" { out.put_bool(&key, v.parse::<bool>().unwrap()); }
          else if ctype == "string" { out.put_str(&key, v); }
          else if ctype == "object" { 
            out.put_object(&key, DataObject::from_json(serde_json::from_str(v).unwrap(), self.env)); 
          }
          else if ctype == "array" { 
            out.put_list(&key, DataArray::from_json(serde_json::from_str(v).unwrap(), self.env)); 
          }
          else { out.put_null(v); }
        }
      }  
      else if cmd_type == "command" {
        let cmdstr = cmd.get_string("cmd");
        let sa = cmdstr.split(":").collect::<Vec<&str>>();
        let lib = sa[0];
        let cmdname = sa[2];
        let mut params = DataObject::new(self.env);
        for (key,v) in in1.duplicate() {
          params.set_property(&key, v);
        }
        
        // FIXME - add remote command support
        // if cmd.has("uuid") {}
        // else {

        let subcmd = Command::new(lib, cmdname, self.env);
        let result = subcmd.execute(params)?;
        
        // FIXME - mapped by order, not by name
        let mut i = 0;
        let cmdout = cmd.get_object("out");
        let keys = subcmd.src.get_object("output").keys();
        for (key1, _v) in cmdout {
          let key2 = &keys[i];
          let dp = result.get_property(key2);
          out.set_property(&key1, dp);
          i = i + 1;
        }
      }
      else if cmd_type == "match" {
        let key = &in1.duplicate().keys()[0];
        let ctype = cmd.get_string("ctype");
        let dp1 = &in1.get_property(key);
        
        // FIXME - Support match on null?
        if ctype == "int" {
          if !dp1.is_int() { b = false; }
          else {
            let val1 = dp1.int();
            let val2 = v.parse::<i64>().unwrap();
            b = val1 == val2;
          }
        }
        else if ctype == "decimal" {
          if !dp1.is_float() { b = false; }
          else {
            let val1 = dp1.float();
            let val2 = v.parse::<f64>().unwrap();
            b = val1 == val2;
          }
        }
        else if ctype == "boolean" {
          if !dp1.is_boolean() { b = false; }
          else {
            let val1 = dp1.boolean();
            let val2 = v.parse::<bool>().unwrap();
            b = val1 == val2;
          }
        }
        else if ctype == "string" {
          if !dp1.is_string() { b = false; }
          else {
            let val1 = dp1.string();
            b = val1 == v.to_owned(); 
          }
        }
        else {
          // FIXME - Objects & Arrays can't match a constant?
          b = false;
        }
        
      }
      else {
        println!("UNIMPLEMENTED OPERATION TYPE {}", cmd_type);
      }
      Ok(())
    })();
    
    if let Err(e) = evaluation {
      if e == CodeException::Fail {
        b = false;
      }
      else {
        return Err(e);
      }
    }
    
    if cmd_type != "constant" && cmd.has("condition") {
      let condition = cmd.get_object("condition");
      self.evaluate_conditional(condition, b)?;
    }

    cmd.put_object("out", out.duplicate());
    cmd.put_bool("done", true);
    
    Ok(out)
  }
  
  fn evaluate_conditional(&mut self, condition:DataObject, m:bool) -> Result<(), CodeException> {
    let rule = condition.get_string("rule");
    let b = condition.get_bool("value");
    if b == m {
      if rule == "next" { return Err(CodeException::NextCase); }
      if rule == "terminate" { return Err(CodeException::Terminate); }
      if rule == "fail" { return Err(CodeException::Fail); }
      if rule == "finish" { self.finishflag = true; }
    }
    
    Ok(())
  }
}


