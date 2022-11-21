use std::cmp;

use crate::primitives::Primitive;
use ndata::dataobject::*;
use ndata::dataarray::*;
use ndata::databytes::*;
use ndata::data::*;
use crate::command::*;
use crate::case::*;
use crate::datastore::*;

#[cfg(not(feature="serde_support"))]
use ndata::json_util::*;

#[derive(PartialEq, Debug)]
pub enum CodeException {
    Fail,
    Terminate,
    NextCase,
}

#[derive(Debug)]
pub struct Code {
  pub data: Case,
  pub finishflag: bool,
}

impl Code {
  pub fn new(data: Case) -> Code {
    Code {
      data: data,
      finishflag: false,
    }
  }

  pub fn execute(&mut self, args: DataObject) -> Result<DataObject, CodeException> {
    let mut out = DataObject::new();
    
    let mut done = false;
    let mut current_case = self.data.duplicate();
    
    while !done {
      let evaluation: Result<(), CodeException> = (|| {
        let cmds = &mut current_case.cmds;
        let n2 = cmds.len();
        let cons = &mut current_case.cons;
        let n = cons.len();
        
        let mut i = 0;
        while i<n2{
          let cmd = &mut cmds.get_mut(i).unwrap();
          if !cmd.done {
            let mut count = 0;
            let mut b = true;
            let input = &mut cmd.input;
            for (key,value) in input {
              count = count + 1;
              if let Some(_con) = self.lookup_con(cons, &key, "in"){
                b = false;
                break;
              }
              else {
                value.done = true;
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
            let con = &mut cons[i];
            if !con.done {
              c = false;
              let mut b = false;
              let mut val = Data::DNull;
              let ja = &mut con.src;
              let src = ja.index;
              let srcname = &ja.name;
              let ja = &mut con.dest;
              let dest = ja.index;
              let destname = &ja.name;
              if src == -1 {
                if args.has(&srcname){
                  val = args.get_property(&srcname);
                }
                b = true;
              }
              else {
                let cmd = &mut cmds[src as usize];
                if cmd.done {
                  val = cmd.result.as_ref().unwrap().get_property(srcname).clone();
                  b = true;
                }
              }
              
              if b {
                con.done = true;
                if dest == -2 {
                  out.set_property(&destname, val);
                }
                else {
                  let cmd = &mut cmds[dest as usize];
                  if cmd.cmd_type == "undefined" {
                    // FIXME - is this used?
                    println!("Marking undefined command as done");
                    cmd.done = true;
                  }
                  else {
                    let var = &mut cmd.input.get_mut(destname).unwrap();
                    var.val = val;
                    var.done = true;
                    
                    let input = &mut cmd.input;
                    for (_key,v) in input {
                      b = b && v.done;
                      if !b { break; }
                    }
                    if b { 
                      self.evaluate(cmd)?; 
                    }
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
      
      if let Err(e) = evaluation {
        if e == CodeException::NextCase {
          current_case = *current_case.nextcase.unwrap();
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
    
  fn lookup_con<'m>(&self, cons: &mut Vec<Connection>, key: &str, which: &str) -> Option<usize> {
    let n = cons.len();
    let mut j = 0;
    while j<n{
      let con = &mut cons.get(j).unwrap();
      let mut bar = &con.src;
      if which == "in" { bar = &con.dest }
      if bar.name == key {
        return Some(j);
      }
      j = j + 1;
    }
    
    None
  }

  fn evaluate(&mut self, cmd: &mut Operation) -> Result<DataObject, CodeException> {
    let mut in1 = DataObject::new();
    let in2 = &mut cmd.input;
    let mut list_in:Vec<String> = Vec::new();
    for (name,in3) in in2 {
      let dp3 = &mut in3.val;
      in1.set_property(&name, dp3.clone());
      
      if in3.mode == "list" { list_in.push(name.to_string()); }
    }
    
    let mut list_out:Vec<String> = Vec::new();    
    let mut loop_out:Vec<String> = Vec::new();    
    for (name,out3) in &mut cmd.output {
      if out3.mode == "list" { list_out.push(name.to_string()); }
      else if out3.mode == "loop" { loop_out.push(name.to_string()); }    
    }
    
    let n = list_in.len();
    let loopn = loop_out.len();
    if n == 0 && loopn == 0 {
      return self.evaluate_operation(cmd, in1);
    }
    else {
      let mut out3 = DataObject::new();
      for key in &list_out { out3.put_array(&key, DataArray::new()); }
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
      if loopn == 0 && count == 0 {
        //cmd.result = Some(out3.duplicate());
        for (k,_v) in &mut cmd.output {
          if !out3.has(&k) { out3.put_null(&k); }
        }
        cmd.done = true;
      }
      else { loop {
        let mut in3 = DataObject::new();
        let list = in1.clone().keys();
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

        let res = self.evaluate_operation(cmd, in3)?;
        
        for (k,v) in &mut cmd.output {
          let dp = res.get_property(k).clone();
          if list_out.contains(&k) {
            out3.get_array(&k).push_property(dp.clone());
          }
          else {
            out3.set_property(&k, dp.clone());
            if loop_out.contains(&k) {
              let newk = &mut v.looop.as_ref().unwrap();
              in1.set_property(&newk, dp.clone());
            }
          }
        }
        
        if cmd.finish {
          break;
        }
        
        if n>0 {
          i = i + 1;
          if i == count {
            break;
          }
        }
      }}
      
      cmd.result = Some(out3.clone());
      return Ok(out3);
    }
  }

  fn evaluate_operation(&mut self, cmd:&mut Operation, in1:DataObject) -> Result<DataObject, CodeException> {
    let mut out = DataObject::new(); // FIXME - Don't instantiate here, leave unassigned
    let cmd_type = &cmd.cmd_type;
    let mut b = true;
    let v = &cmd.name;
    
    let evaluation: Result<(), CodeException> = (|| {
      if cmd_type == "primitive" {
        let p = Primitive::new(v);
        out = p.execute(in1);
      }
      else if cmd_type == "local" {
/*
        let src = cmd.localdata.as_ref().unwrap();
        let mut holder = DataObject::new();
        let panic_result = panic::catch_unwind(|| {
          let mut holder = DataObject::get(holder.data_ref);
          let mut code = Code::new(src.clone());
          let local_res = code.execute(in1);
          if let Err(e) = local_res {
            if e == CodeException::NextCase { holder.put_string("error", "next"); }
            else if e == CodeException::Terminate { holder.put_string("error", "terminate"); }
            else { holder.put_string("error", "fail"); }
          }
          else {
            let x = local_res.unwrap();
            holder.put_object("result", x);
          }
          holder.put_boolean("finish", code.finishflag);
        });
        
        match panic_result {
          Ok(_x) => (),
          Err(e) => {
//            let s = match e.downcast::<String>() {
//              Ok(panic_msg) => format!("{}", panic_msg),
//              Err(_) => "unknown error".to_string()
//            };   
            return Err(CodeException::Fail);
          }
        }
        
        if holder.has("error") {
          let x = holder.get_string("error");
          if x == "next" { return Err(CodeException::NextCase); }
          else if x == "terminate" { return Err(CodeException::Terminate); }
          return Err(CodeException::Fail);
        }
        
        let x = holder.get_object("result");
        for (k,v) in x.objects() { out.set_property(&k, v); }
        if holder.has("finish") { cmd.finish = holder.get_bool("finish"); }
*/
        if cmd.localdata.is_none() {
          for (key,_x) in &mut cmd.output {
            out.put_null(&key);
          }
        }
        else {
          let src = cmd.localdata.as_ref().unwrap();
          let mut code = Code::new(src.duplicate());
          out = code.execute(in1)?;
          cmd.finish = code.finishflag;
        }
      }
      else if cmd_type == "constant" {
        for (key,_x) in &mut cmd.output {
          let ctype = cmd.ctype.as_ref().unwrap();
          if ctype == "int" { out.put_int(&key, v.parse::<i64>().unwrap()); }
          else if ctype == "decimal" { out.put_float(&key, v.parse::<f64>().unwrap()); }
          else if ctype == "boolean" { out.put_boolean(&key, v.parse::<bool>().unwrap()); }
          
          else if ctype == "string" { 
            #[cfg(not(feature="serde_support"))]
            out.put_string(&key, &unescape(&v)); 
            #[cfg(feature="serde_support")]
            out.put_string(&key, serde_json::from_str(&format!("\"{}\"", &v)).unwrap());
          }
          
          else if ctype == "object" { 
            out.put_object(&key, DataObject::from_string(v)); 
          }
          else if ctype == "array" { 
            out.put_array(&key, DataArray::from_string(v)); 
          }
          else { out.put_null(&key); }
        }
      }  
      else if cmd_type == "command" {
        let cmdstr = cmd.cmd.as_ref().unwrap();
        let sa = cmdstr.split(":").collect::<Vec<&str>>();
        let lib = sa[0];
        let cmdname = sa[2];
        let mut params = DataObject::new();
        for (key,v) in in1.objects() {
          params.set_property(&key, v);
        }
        
        // FIXME - add remote command support
        // if cmd.has("uuid") {}
        // else {

        let subcmd = Command::new(lib, cmdname);
        let result = subcmd.execute(params)?;
        
        // FIXME - mapped by order, not by name
        let mut i = 0;
        let cmdout = &mut cmd.output;
        
        let mut keys = DataArray::new();
        if let Source::Flow(src) = subcmd.src { 
          for k in src.output.keys() { keys.push_string(k); }
        }
        else {
          keys.push_string("a");
        }
//          let src = subcmd.src();
//          for k in src.output.keys() { keys.push(k); }

        for (key1, _v) in cmdout {
          let key2:&str = &keys.get_string(i);
          let dp = result.get_property(key2);
          out.set_property(&key1, dp);
          i = i + 1;
        }
      }
      else if cmd_type == "persistent" {
        let mut g = DataStore::globals();
        let keys = &in1.clone().keys();
        if keys.len() > 0 {
          let key = &keys[0];
          let dp1 = &in1.get_property(key);
          g.set_property(v, dp1.clone());
        }
//        let cmdout = &mut cmd.output;
        let value: Data;
        if g.has(v) {
          value = g.get_property(v);
        }
        else {
          value = Data::DNull;
        }
        for (key,_x) in &mut cmd.output {
          out.set_property(&key, value.clone());
        }
      }
      else if cmd_type == "match" {
        let key = &in1.clone().keys()[0];
        let ctype = cmd.ctype.as_ref().unwrap();
        let dp1 = &in1.get_property(key);
        
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
        else if ctype == "null" {
          b = dp1.clone().is_null();
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

    DataObject::gc();
    DataArray::gc();
    DataBytes::gc();
    
    if let Err(e) = evaluation {
      if e == CodeException::Fail {
        b = false;
      }
      else {
        println!("UNEXPECTED ERROR {:?}", e);
        return Err(e);
      }
    }
    
    if cmd_type != "constant" && !cmd.condition.is_none() {
      let condition = &mut cmd.condition.as_ref().unwrap();
      self.evaluate_conditional(&condition.rule, condition.value, b)?;
    }

    cmd.result = Some(out.clone());
    cmd.done = true;
    
    //println!("OP DONE {} {:?}", cmd_type, out.to_json());
    Ok(out)
  }
  
  fn evaluate_conditional(&mut self, c_rule:&str, c_val:bool, m:bool) -> Result<(), CodeException> {
    if c_val == m {
      if c_rule == "next" { return Err(CodeException::NextCase); }
      if c_rule == "terminate" { return Err(CodeException::Terminate); }
      if c_rule == "fail" { return Err(CodeException::Fail); }
      if c_rule == "finish" { self.finishflag = true; }
    }
    
    Ok(())
  }
}


