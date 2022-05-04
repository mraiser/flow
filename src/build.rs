use std::env;
use std::io;
use std::io::BufRead;

use std::io::Read;
use std::fs::File;
use std::io::Write;
use std::path::*;
use serde_json::Value;
use serde_json::json;
use std::fs::create_dir_all;
use std::fs::OpenOptions;
use std::io::prelude::*;

mod datastore;
use datastore::*;

fn main() {
  DataStore::init("data");

  env::set_var("RUST_BACKTRACE", "1");
  {
    let params: Vec<String> = env::args().collect();
    let lib = &params[1];
    let ctl = &params[2];
    let cmd = &params[3];
    
    build(lib, ctl, cmd);
  }
}

pub fn build(lib:&str, ctl:&str, cmd:&str) {
  let store = DataStore::new();
  let id = &store.lookup_cmd_id(lib, ctl, cmd);

  let meta = store.get_json(lib, id);
  let data = &meta["data"];
  let typ = &data["type"];
  
  if typ == "rust" {
    let id = data["rust"].as_str().unwrap();
    let mut meta = store.get_json(lib, id);
    
    let path = store.get_data_file(lib, &(id.to_owned()+".rs"));
    let src = store.read_file(path);
    let path = store.root.parent().unwrap().join("src").join("generated").join(lib).join(ctl);
    
    meta["lib"] = json!(lib);
    meta["ctl"] = json!(ctl);
    meta["cmd"] = json!(cmd);
    
    // FIXME - Don't recompile if current.
    // FIXME - Cache in global state/Storage
    // FIXME - Actually compile the specified code with imports
    
    build_rust(path, meta, &src);
  }
}

fn lookup_type(t:&str) -> String {
  let typ = match t {
    "Integer" => "i64",
    _ => "DataObject"
  };
  typ.to_string()
}

fn file_contains(path2:&PathBuf, m:&str) -> bool{
  let mut file = File::open(&path2).unwrap();
  let lines = io::BufReader::new(file).lines();
  for line in lines {
    if let Ok(ip) = line {
      if ip == m {
        return true;
      }
    }
  }
  false
}

fn build_mod(path2:&PathBuf, m:&str) {
  if path2.exists() {
    if !file_contains(path2, m) {
      let mut file = OpenOptions::new()
        .write(true)
        .append(true)
        .open(&path2)
        .unwrap();
      file.write_all(b"\n");
      file.write_all(m.as_bytes());
    }
  }
  else {
    let mut file = File::create(&path2).unwrap();
    file.write_all(m.as_bytes());
  }
}

fn build_rust(path:PathBuf, meta:Value, src:&str) {
  create_dir_all(&path);
  let id = meta["id"].as_str().unwrap();
  let lib = meta["lib"].as_str().unwrap();
  let ctl = meta["ctl"].as_str().unwrap();
  let cmd = meta["cmd"].as_str().unwrap();
  let data = &meta["data"];
  let import = data["import"].as_str().unwrap();
  let returntype = &lookup_type(data["returntype"].as_str().unwrap());
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
    let typ = &lookup_type(t);
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
  
  let m = "pub mod ".to_string()+cmd+";";
  let path2 = &path.join("mod.rs");
  build_mod(path2, &m);  
  
  let m = "pub mod ".to_string()+ctl+";";
  let path2 = &path2.parent().unwrap().parent().unwrap().join("mod.rs");
  build_mod(path2, &m);  
  
  let m = "      \"".to_string()+id+"\" => "+lib+"::"+ctl+"::"+cmd+"::execute,";
  let mm = "pub mod ".to_string()+lib+";\n";
  let path2 = &path2.parent().unwrap().parent().unwrap().join("mod.rs");
  if path2.exists() {
    let mut file = File::open(&path2).unwrap();
    let lines = io::BufReader::new(file).lines();
    let mut part1 = Vec::<String>::new();
    let mut part2 = Vec::<String>::new();
    let mut b = true;
    let mut c = true;
    let begin = "    match name {";
    for line in lines {
      if let Ok(ip) = line {
        if ip == m {
          b = false;
          break;
        }
        
        if c {
          part1.push(ip.to_string());
        }
        else {
          part2.push(ip.to_string());
        }
        
        if ip == begin {
          c = false;
        }
      }
    }
    if b {
      let mut file = File::create(&path2).unwrap();
      file.write_all(mm.as_bytes());
      for line in part1 {
        file.write_all(line.as_bytes());
        file.write_all(b"\n");
      }
      file.write_all(m.as_bytes());
      file.write_all(b"\n");
      for line in part2 {
        file.write_all(line.as_bytes());
        file.write_all(b"\n");
      }
    }
    println!("{}",b);
    println!("{}",m);
  }
  else {
      let mut file = File::create(&path2).unwrap();
      file.write_all(mm.as_bytes());
      file.write_all(b"use crate::rustcmd::*;\n");
      file.write_all(b"pub struct Generated {}\n");
      file.write_all(b"impl Generated {\n");
      file.write_all(b"  pub fn get(name:&str) -> Transform {\n");
      file.write_all(b"    match name {\n");
      file.write_all(m.as_bytes());
      file.write_all(b"\n");
      file.write_all(b"      _ => { panic!(\"No such rust command {}\", name); }\n");
      file.write_all(b"    }\n");
      file.write_all(b"  }\n");
      file.write_all(b"}\n");
  }
  
/*    
  let mut compile_file = Command::new("rustc");
  let newfile = path2.to_str().unwrap();
  
  let path3 = &path.join("lib".to_string()+id+".so");
  remove_file(path3);

  let x = compile_file.args(&["-A", "dead_code", "-A", "unused_imports", "--out-dir", &path.to_str().unwrap(), "--crate-type", "cdylib", "-L", "target/release/deps", "--extern", "ndata=../ndata/target/release/libndata.rlib", "--edition=2021", &newfile]).status().expect("process failed to execute");
  
  println!("{:?}", x);
*/
}

