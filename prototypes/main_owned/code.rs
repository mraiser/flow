use std::cmp;

use crate::flowenv::*;
use crate::primitives::Primitive;
use crate::dataobject::*;
use crate::dataarray::*;
use crate::data::*;
use crate::command::Command;

#[derive(PartialEq, Debug)]
pub enum CodeException {
    Fail,
    Terminate,
    NextCase,
}

#[derive(Debug)]
pub struct Code {
  pub data: DataObject,
  pub finishflag: bool,
}

impl Code {
  pub fn new(data: DataObject) -> Code {
    Code {
      data: data,
      finishflag: false,
    }
  }

  pub fn execute(&mut self, args: DataObject, env:&mut FlowEnv) -> Result<DataObject, CodeException> {
    let mut done = false;
    let mut out = DataObject::new(env);
    
    let mut current_case = self.data.duplicate(env);
    
    while !done {
      let evaluation: Result<(), CodeException> = (|| {
        let cmds = current_case.get_array("cmds", env);
        let n2 = cmds.len(env);
        let cons = current_case.get_array("cons", env);
        let n = cons.len(env);
        
        let mut i = 0;
        while i<n2{
          let mut cmd = cmds.get_object(i, env);
          if !cmd.has("done", env) { cmd.put_bool("done", false, env); }
          if !cmd.get_bool("done", env) {
            let mut count = 0;
            let mut b = true;
            for (key,_value) in cmd.get_object("in", env).objects(env) {
              count = count + 1;
              if let Some(_con) = self.lookup_con(&cons, &key, "in", env){
                b = false;
                break;
              }
              else {
                //println!("No input found!");
                cmd.get_object(&key, env).put_bool("done", true, env);
              }
            }
            if count == 0 || b {
              self.evaluate(cmd, env)?;
            }
          }
          i = i + 1;
        }
        
        while !done {
          let mut c = true;
          let mut i = 0;
          while i<n {
            let mut con = cons.get_object(i, env);
            if !con.has("done", env) { con.put_bool("done", false, env); }
            if !con.get_bool("done", env) {
              c = false;
              let mut b = false;
              let mut val = Data::DNull;
              let ja = con.get_array("src", env);
              let src = ja.get_i64(0, env);
              let srcname = ja.get_string(1, env);
              let ja = con.get_array("dest", env);
              let dest = ja.get_i64(0, env);
              let destname = ja.get_string(1, env);
              if src == -1 {
                if args.has(&srcname, env){
                  val = args.get_property(&srcname, env);
                }
                b = true;
              }
              else {
                let cmd = cmds.get_object(src as usize, env);
                if cmd.get_bool("done", env) {
                  //println!("SRCNAME {}", &srcname);
                  val = cmd.get_object("out", env).get_property(&srcname, env);
                  b = true;
                }
              }
              
              if b {
                con.put_bool("done", true, env);
                if dest == -2 {
                  out.set_property(&destname, val, env);
                }
                else {
                  let mut cmd = cmds.get_object(dest as usize, env);
                  if cmd.get_string("type", env) == "undefined" {
                    // FIXME - is this used?
                    println!("Marking undefined command as done");
                    cmd.put_bool("done", true, env);
                  }
                  else {
                    let mut var = cmd.get_object("in", env).get_object(&destname, env);
                    var.set_property("val", val, env);
                    var.put_bool("done", true, env);
                    
                    let input = cmd.get_object("in", env);
                    for (_key,v) in input.objects(env) {
                      let mut value = v.object(env);
                      if !value.has("done", env) { value.put_bool("done", false, env); }
                      b = b && value.get_bool("done", env);
                      if !b { break; }
                    }
                    if b { self.evaluate(cmd, env)?; }
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
        Ok(())
      })();
      
      env.gc();
      
      if let Err(e) = evaluation {
        if e == CodeException::NextCase {
          current_case = current_case.get_object("nextcase", env);
        }
        else if e == CodeException::Terminate {
          break;
        }
        else {
          return Err(e);
        }
      }
      
    }
    
    Ok(out)
  }
    
  fn lookup_con<'m>(&self, cons: &DataArray, key: &str, which: &str, env:&mut FlowEnv) -> Option<DataObject> {
    let n = cons.len(env);
    let mut j = 0;
    while j<n{
      let con = cons.get_object(j, env);
      let mut bar = con.get_array("src", env);
      if which == "in" { bar = con.get_array("dest", env) }
      if bar.get_string(1, env) == key {
        return Some(con);
      }
      j = j + 1;
    }
    
    None
  }

  fn evaluate(&mut self, mut cmd: DataObject, env:&mut FlowEnv) -> Result<DataObject, CodeException> {
    let mut in1 = DataObject::new(env);
    let in2 = cmd.get_object("in", env);
    let mut list_in:Vec<String> = Vec::new();
    for (name,v) in in2.objects(env) {
      let in3 = v.object(env);
      
      let dp3 = in3.get_property("val", env);
      in1.set_property(&name, dp3, env);
      
      if in3.has("mode", env) && in3.get_string("mode", env) == "list" { list_in.push(name); }
    }
    
    let out2 = cmd.get_object("out", env);
    let mut list_out:Vec<String> = Vec::new();    
    let mut loop_out:Vec<String> = Vec::new();    
    for (name,v) in out2.objects(env) {
      let out3 = v.object(env);
      if out3.has("mode", env) {
        let mode = out3.get_string("mode", env);
        if mode == "list" { list_out.push(name); }
        else if mode == "loop" { loop_out.push(name); }    
      }
    }
    
    let n = list_in.len();
    if n == 0 && loop_out.len() == 0 {
      return self.evaluate_operation(cmd, in1, env);
    }
    else {
      let mut out3 = DataObject::new(env);
      for key in &list_out { out3.put_list(&key, DataArray::new(env), env); }
      let mut count = 0;
      if n>0 {
        count = in1.get_array(&list_in[0], env).len(env);
        let mut i = 1;
        while i<n {
          count = cmp::min(count, in1.get_array(&list_in[i], env).len(env));
          i = i + 1;
        }
      }
      
      let mut i = 0;
      loop {
        let mut in3 = DataObject::new(env);
        let list = in1.duplicate(env).keys(env);
        for key in list {
          if !list_in.contains(&key) { 
            let dp = in1.get_property(&key, env);
            in3.set_property(&key, dp, env); 
          }
          else {
            let ja = in1.get_array(&key, env);
            let dp = ja.get_property(i, env);
            in3.set_property(&key, dp, env); 
          }
        }

        self.evaluate_operation(cmd.duplicate(env), in3, env)?;
        
        let out = cmd.get_object("out", env);
        for (k,_v) in out2.objects(env) {
          if out.has(&k, env) {
            let dp = out.get_property(&k, env);
            if list_out.contains(&k) {
              out3.get_array(&k, env).push_property(dp, env);
            }
            else {
              out3.set_property(&k, dp.clone(), env);
              if loop_out.contains(&k) {
                let newk = out2.get_object(&k, env).get_string("loop", env);
                in1.set_property(&newk, dp.clone(), env);
              }
            }
          }
        }
        
        env.gc();
        
        if cmd.has("FINISHED", env) && cmd.get_bool("FINISHED", env) {
          break;
        }
        
        if n>0 {
          i = i + 1;
          if i == count {
            break;
          }
        }
      }
      
      cmd.put_object("out", out3.duplicate(env), env);
      return Ok(out3);
    }
  }

  fn evaluate_operation(&mut self, mut cmd:DataObject, in1:DataObject, env:&mut FlowEnv) -> Result<DataObject, CodeException> {
    let mut out = DataObject::new(env); // FIXME - Don't instantiate here, leave unassigned
    let cmd_type = cmd.get_string("type", env);
    let mut b = true;
    let v = &cmd.get_string("name", env);
    
    let evaluation: Result<(), CodeException> = (|| {
      if cmd_type == "primitive" { // FIXME - use match
        let p = Primitive::new(v, env);
        out = p.execute(in1, env);
      }
      else if cmd_type == "local" {
        let src = cmd.get_object("localdata", env);
        let mut code = Code::new(src.deep_copy(env));
        out = code.execute(in1, env)?;
        cmd.put_bool("FINISHED", code.finishflag, env);
      }
      else if cmd_type == "constant" {
        for (key,_x) in cmd.get_object("out", env).objects(env) {
          let ctype = cmd.get_string("ctype", env);
          if ctype == "int" { out.put_i64(&key, v.parse::<i64>().unwrap(), env); }
          else if ctype == "decimal" { out.put_float(&key, v.parse::<f64>().unwrap(), env); }
          else if ctype == "boolean" { out.put_bool(&key, v.parse::<bool>().unwrap(), env); }
          else if ctype == "string" { out.put_str(&key, v, env); }
          else if ctype == "object" { 
            out.put_object(&key, DataObject::from_json(serde_json::from_str(v).unwrap(), env), env); 
          }
          else if ctype == "array" { 
            out.put_list(&key, DataArray::from_json(serde_json::from_str(v).unwrap(), env), env); 
          }
          else { out.put_null(v, env); }
        }
      }  
      else if cmd_type == "command" {
        let cmdstr = cmd.get_string("cmd", env);
        let sa = cmdstr.split(":").collect::<Vec<&str>>();
        let lib = sa[0];
        let cmdname = sa[2];
        let mut params = DataObject::new(env);
        for (key,v) in in1.objects(env) {
          params.set_property(&key, v, env);
        }
        
        // FIXME - add remote command support
        // if cmd.has("uuid") {}
        // else {

        let subcmd = Command::new(lib, cmdname, env);
        let result = subcmd.execute(params, env)?;
        
        // FIXME - mapped by order, not by name
        let mut i = 0;
        let cmdout = cmd.get_object("out", env);
        let keys = subcmd.src.get_object("output", env).keys(env);
        for (key1, _v) in cmdout.objects(env) {
          let key2 = &keys[i];
          let dp = result.get_property(key2, env);
          out.set_property(&key1, dp, env);
          i = i + 1;
        }
      }
      else if cmd_type == "match" {
        let key = &in1.duplicate(env).keys(env)[0];
        let ctype = cmd.get_string("ctype", env);
        let dp1 = &in1.get_property(key, env);
        
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
    
    env.gc();
    
    if let Err(e) = evaluation {
      if e == CodeException::Fail {
        b = false;
      }
      else {
        return Err(e);
      }
    }
    
    if cmd_type != "constant" && cmd.has("condition", env) {
      let condition = cmd.get_object("condition", env);
      self.evaluate_conditional(condition, b, env)?;
    }

    cmd.put_object("out", out.duplicate(env), env);
    cmd.put_bool("done", true, env);
    
    Ok(out)
  }
  
  fn evaluate_conditional(&mut self, condition:DataObject, m:bool, env:&mut FlowEnv) -> Result<(), CodeException> {
    let rule = condition.get_string("rule", env);
    let b = condition.get_bool("value", env);
    if b == m {
      if rule == "next" { return Err(CodeException::NextCase); }
      if rule == "terminate" { return Err(CodeException::Terminate); }
      if rule == "fail" { return Err(CodeException::Fail); }
      if rule == "finish" { self.finishflag = true; }
    }
    
    Ok(())
  }
}


