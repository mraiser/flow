use ndata::dataobject::*;
use std::process::Command;
use std::io::Write;
use std::process::Stdio;

use crate::code::*;
use crate::datastore::*;

#[derive(Debug)]
pub struct PyCmd {
  lib:String,
  id:String,
}

impl PyCmd{
  pub fn new(lib:&str, id:&str) -> PyCmd{
    PyCmd{
      lib:lib.to_string(),
      id:id.to_string(),
    }
  }
  
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    let store = DataStore::new();
    let f = store.root.to_owned();
    let f = f.canonicalize().unwrap();
    let f = f.parent().unwrap();
    let f = f.join("generated");
    let f = f.join("com");
    let f = f.join("newbound");
    let f = f.join("robot");
    let f = f.join("published");
    let f = f.join(self.lib.to_owned());
    let f = f.join(self.id.to_owned()+".py");
    
    let mut cmd = Command::new("python3")
            .arg(f)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to execute process");
    
    let sin = cmd.stdin.as_mut().unwrap();
    sin.write_all(args.to_json().to_string().as_bytes());
    drop(sin);
    let output = cmd.wait_with_output().unwrap();
    let output = std::str::from_utf8(&output.stdout).unwrap();
    
    Ok(DataObject::from_json(serde_json::from_str(output).unwrap()))
  }
}

