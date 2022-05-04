use std::fs::File;
use std::io::Write;
use std::path::*;
use serde_json::Value;
//use std::process::Command;
use std::fs::create_dir_all;
//use std::fs::remove_file;

use ndata::dataobject::*;

use crate::code::*;

pub type Transform = fn(DataObject) -> DataObject;

#[derive(Debug)]
pub struct RustCmd {
  func:Transform,
}

impl RustCmd{
  pub fn new(t:Transform) -> RustCmd{
    RustCmd{
      func:t,
    }
  }
  
  pub fn execute(&self, args:DataObject) -> Result<DataObject, CodeException> {
    Ok((self.func)(args))
  }
  
  fn lookup_type(t:&str) -> String {
    let typ = match t {
      "Integer" => "i64",
      _ => "DataObject"
    };
    typ.to_string()
  }
  
  pub fn build_rust(path:PathBuf, meta:Value, src:&str) {
    create_dir_all(&path);
    let cmd = meta["cmd"].as_str().unwrap();
    let data = &meta["data"];
    let import = data["import"].as_str().unwrap();
    let returntype = &RustCmd::lookup_type(data["returntype"].as_str().unwrap());
    let params = &data["params"];
    
    let path2 = &path.join(cmd.to_string()+".rs");
    let mut file = File::create(&path2).unwrap();

    file.write_all(b"use ndata::dataobject::*;\n");
    file.write_all(import.as_bytes());
    file.write_all(b"\npub fn execute(o: DataObject) -> DataObject {\n");
    
    let mut index = 0;
    let mut invoke1 = "let ax = ".to_string()+cmd+"(";
    let mut invoke2 = "pub fn ".to_string()+cmd+"(";
    for v in params.as_array().unwrap() {
      let name = v["name"].as_str().unwrap();
      let t = v["type"].as_str().unwrap();
      let typ = &RustCmd::lookup_type(t);
      //println!("{} / {}", name, typ);
      let line = "let a".to_string() + &index.to_string() + " = o.get_" + typ + "(\"" + name + "\");\n";
      file.write_all(line.as_bytes());
      if index > 0 {
        invoke1 = invoke1 + ", ";
        invoke2 = invoke2 + ", ";
      }
      invoke1 = invoke1 + "a" + &index.to_string();
      invoke2 = invoke2 + name + ":" + typ;
      index += 1;
    }
    invoke1 = invoke1 + ");\n";
    invoke2 = invoke2 + ") -> " + returntype + " {\n";

    file.write_all(invoke1.as_bytes());
    file.write_all(b"let mut o = DataObject::new();\n");
    file.write_all(b"o.put_");
    file.write_all(returntype.as_bytes());
    file.write_all(b"(\"a\", ax);\n");
    file.write_all(b"o\n");
    file.write_all(b"}\n\n");
    file.write_all(invoke2.as_bytes());
    file.write_all(src.as_bytes());
    file.write_all(b"}\n\n");
    
    
    
/*    
    let mut compile_file = Command::new("rustc");
    let newfile = path2.to_str().unwrap();
    
    let path3 = &path.join("lib".to_string()+id+".so");
    remove_file(path3);

    let x = compile_file.args(&["-A", "dead_code", "-A", "unused_imports", "--out-dir", &path.to_str().unwrap(), "--crate-type", "cdylib", "-L", "target/release/deps", "--extern", "ndata=../ndata/target/release/libndata.rlib", "--edition=2021", &newfile]).status().expect("process failed to execute");
    
    println!("{:?}", x);
*/
  }
}

