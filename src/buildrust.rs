use std::env;
use std::io;
use std::io::BufRead;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::*;
use serde_json::Value;
use serde_json::json;
use std::fs::create_dir_all;
use std::fs::OpenOptions;

use crate::datastore::*;

pub fn buildAll() {
  let libs = fs::read_dir("data").unwrap();
  let store = DataStore::new();
  for db in libs {
    let lib = db.unwrap().file_name().into_string().unwrap();
    if !store.exists(&lib, "controls") {
      println!("No controls in library {}", &lib);
    }
    else {
      let controls = store.get_json(&lib, "controls");
      let list = controls["data"]["list"].as_array().unwrap();
      for control in list {
        let ctl = (&control["name"]).as_str().unwrap();
        let id = (&control["id"]).as_str().unwrap();
        if !store.exists(&lib, &id) {
          println!("No control file for {}:{}", &lib, &id);
        }
        else {
          let ctldata = store.get_json(&lib, &id);
          let cmdlist = &ctldata["data"]["cmd"];
          if !cmdlist.is_null() {
            let cmdlist = ctldata["data"]["cmd"].as_array().unwrap();
            for command in cmdlist {
              let cmd = (&command["name"]).as_str().unwrap();
              build(&lib, &ctl, &cmd);
            }
          }
        }
      }
    }
  }    
}

pub fn build(lib:&str, ctl:&str, cmd:&str) {
  let store = DataStore::new();
  let id = &store.lookup_cmd_id(lib, ctl, cmd);
  
  if store.exists(lib, id) {
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
      
      // FIXME - Don't rebuild if current
      
      println!("Building Rust: {}:{}:{}", lib, ctl, cmd);
      build_rust(path, meta, &src);
    }
  }
}

fn lookup_type(t:&str) -> String {
  let typ = match t {
    "Any" => "Data",
    "Integer" => "i64",
    "Float" => "f64",
    "String" => "String",
    "Boolean" => "bool",
    "JSONArray" => "DataArray",
    _ => "DataObject"
  };
  typ.to_string()
}

fn file_contains(path2:&PathBuf, m:&str) -> bool{
  let file = File::open(&path2).unwrap();
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
      let _x = file.write_all(b"\n");
      let _x = file.write_all(m.as_bytes());
    }
  }
  else {
    let mut file = File::create(&path2).unwrap();
    let _x = file.write_all(m.as_bytes());
  }
}

fn build_rust(path:PathBuf, meta:Value, src:&str) {
  let _x = create_dir_all(&path);
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

  let _x = file.write_all(b"use ndata::dataobject::*;\n");
  let _x = file.write_all(b"use ndata::data::*;\n");
  let _x = file.write_all(import.as_bytes());
  let _x = file.write_all(b"\npub fn execute(o: DataObject) -> DataObject {\n");
  
  let mut index = 0;
  let mut invoke1 = "let ax = ".to_string()+cmd+"(";
  let mut invoke2 = "pub fn ".to_string()+cmd+"(";
  for v in params.as_array().unwrap() {
    let name = v["name"].as_str().unwrap();
    let t = v["type"].as_str().unwrap();
    let typ = &lookup_type(t);
    let typ2;
    if typ == "DataObject" { typ2 = "object".to_string(); }
    else if typ == "DataArray" { typ2 = "array".to_string(); }
    else if typ == "Data" { typ2 = "property".to_string(); }
    else { typ2 = typ.to_lowercase(); }
    //println!("{} / {}", name, typ);
    let line = "let a".to_string() + &index.to_string() + " = o.get_" + &typ2 + "(\"" + name + "\");\n";
    let _x = file.write_all(line.as_bytes());
    if index > 0 {
      invoke1 = invoke1 + ", ";
      invoke2 = invoke2 + ", ";
    }
    invoke1 = invoke1 + "a" + &index.to_string();
    invoke2 = invoke2 + "mut " + name + ":" + typ;
    index += 1;
  }
  invoke1 = invoke1 + ");\n";
  invoke2 = invoke2 + ") -> " + returntype + " {\n";

  let _x = file.write_all(invoke1.as_bytes());
  let _x = file.write_all(b"let mut o = DataObject::new();\n");
  if returntype == "Data" {
    let _x = file.write_all(b"o.set_property(\"a\", ax);\n");
  }
  else {
    let _x = file.write_all(b"o.put_");
    if returntype == "String" {
      let _x = file.write_all(b"str");
      let _x = file.write_all(b"(\"a\", &ax);\n");
    }
    else if returntype == "f64" {
      let _x = file.write_all(b"float");
      let _x = file.write_all(b"(\"a\", ax);\n");
    }
    else if returntype == "DataObject" {
      let _x = file.write_all(b"object");
      let _x = file.write_all(b"(\"a\", ax);\n");
    }
    else if returntype == "DataArray" {
      let _x = file.write_all(b"list");
      let _x = file.write_all(b"(\"a\", ax);\n");
    }
    else {
      let _x = file.write_all(returntype.as_bytes());
      let _x = file.write_all(b"(\"a\", ax);\n");
    }
  }
  let _x = file.write_all(b"o\n");
  let _x = file.write_all(b"}\n\n");
  let _x = file.write_all(invoke2.as_bytes());
  let _x = file.write_all(src.as_bytes());
  let _x = file.write_all(b"\n}\n\n");
  
  let m = "pub mod ".to_string()+cmd+";";
  let path2 = &path.join("mod.rs");
  build_mod(path2, &m);  
  
  let m = "pub mod ".to_string()+ctl+";";
  let path2 = &path2.parent().unwrap().parent().unwrap().join("mod.rs");
  build_mod(path2, &m);  
  
  let m = "    RustCmd::add(\"".to_string()+id+"\".to_string(), "+lib+"::"+ctl+"::"+cmd+"::execute, \"\".to_string());";
  let mm = "pub mod ".to_string()+lib+";";
  let path2 = &path2.parent().unwrap().parent().unwrap().join("mod.rs");
  if path2.exists() {
    let file = File::open(&path2).unwrap();
    let lines = io::BufReader::new(file).lines();
    let mut part1 = Vec::<String>::new();
    let mut part2 = Vec::<String>::new();
    let mut a = true;
    let mut b = true;
    let mut c = true;
    let begin = "    RustCmd::init();";
    for line in lines {
      if let Ok(ip) = line {
        if ip == m {
          b = false;
          break;
        }
        
        if ip == mm {
          a = false;
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
      if a {
        let _x = file.write_all(mm.as_bytes());
        let _x = file.write_all(b"\n");
      }
      for line in part1 {
        let _x = file.write_all(line.as_bytes());
        let _x = file.write_all(b"\n");
      }
      let _x = file.write_all(m.as_bytes());
      let _x = file.write_all(b"\n");
      for line in part2 {
        let _x = file.write_all(line.as_bytes());
        let _x = file.write_all(b"\n");
      }
    }
//    println!("{}",b);
//    println!("{}",m);
  }
  else {
      let mut file = File::create(&path2).unwrap();
      let _x = file.write_all(mm.as_bytes());
      let _x = file.write_all(b"\n");
      let _x = file.write_all(b"use flowlang::rustcmd::*;\n");
      let _x = file.write_all(b"pub struct Generated {}\n");
      let _x = file.write_all(b"impl Generated {\n");
      let _x = file.write_all(b"  pub fn init() {\n");
      let _x = file.write_all(b"    RustCmd::init();\n");
      let _x = file.write_all(m.as_bytes());
      let _x = file.write_all(b"\n");
      let _x = file.write_all(b"  }\n");
      let _x = file.write_all(b"}\n");
  }
}

