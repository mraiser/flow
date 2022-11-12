use std::io;
use std::io::BufRead;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::*;
use std::fs::create_dir_all;
use std::fs::OpenOptions;
use std::io::BufReader;
use ndata::dataobject::*;

use crate::datastore::*;

pub fn build_all() -> bool {
  let mut b = false;
  let libs = fs::read_dir("data").unwrap();
  for db in libs {
    let lib = db.unwrap().file_name().into_string().unwrap();
    b = b || build_lib(lib);
  }
  b
}

pub fn build_lib(lib:String) -> bool {
  let mut b = false;
  let store = DataStore::new();
  let root = store.get_lib_root(&lib);
  if !store.exists(&lib, "controls") {
    println!("No controls in library {}", &lib);
  }
  else {
    let controls = store.get_data(&lib, "controls");
    let list = controls.get_object("data").get_array("list");
    for control in list.objects() {
      let control = control.object();
      let ctl = control.get_string("name");
      let id = control.get_string("id");
      if !store.exists(&lib, &id) {
        println!("No control file for {}:{}", &lib, &id);
      }
      else {
        let ctldata = store.get_data(&lib, &id);
        let d = ctldata.get_object("data");
        if d.has("cmd") {
          let cmdlist = d.get_array("cmd");
          for command in cmdlist.objects() {
            let command = command.object();
            let cmd = command.get_string("name");
            b = b || build(&lib, &ctl, &cmd, &root);
          }
        }
      }
    }
  }
  b
}

pub fn build(lib:&str, ctl:&str, cmd:&str, root:&Path) -> bool {
  let mut b = false;
  let store = DataStore::new();
  let id = &store.lookup_cmd_id(lib, ctl, cmd);
  
  if store.exists(lib, id) {
    let meta = store.get_data(lib, id);
    let data = meta.get_object("data");
    let typ = data.get_string("type");
    
    if typ == "rust" {
      let id = &data.get_string("rust");
      let mut meta = store.get_data(lib, id);
      
      let path = store.get_data_file(lib, &(id.to_owned()+".rs"));
      let src = store.read_file(path);
      let path = root.join("src").join(lib).join(ctl);
      
      meta.put_str("lib", lib);
      meta.put_str("ctl", ctl);
      meta.put_str("cmd", cmd);
      
      // FIXME - Don't rebuild if current
      
      println!("Building Rust: {}:{}:{}", lib, ctl, cmd);
      build_rust(path, meta, &src);
      b = true;
    }
    else if typ == "python" {
      let cid = &data.get_string("python");
      let mut meta = store.get_data(lib, cid);
      
      let path = store.get_data_file(lib, &(cid.to_owned()+".python"));
      
      let src = store.read_file(path);
      let pypath = store.root.parent().unwrap().join("lib_python");
      let path = store.root.parent().unwrap()
                                    .join("generated")
                                    .join("com")
                                    .join("newbound")
                                    .join("robot")
                                    .join("published")
                                    .join(lib);
      
      meta.put_str("ctlid", id);
      meta.put_str("lib", lib);
      meta.put_str("ctl", ctl);
      meta.put_str("cmd", cmd);
      
      // FIXME - Don't rebuild if current
      
      println!("Building Python: {}:{}:{}", lib, ctl, cmd);
      build_python(pypath, path, meta, &src);
    }
  }
  b
}

fn lookup_type(t:&str) -> String {
  let typ = match t {
    "Any" => "Data",
    "Integer" => "i64",
    "Float" => "f64",
    "String" => "String",
    "File" => "String",
    "Boolean" => "bool",
    "JSONArray" => "DataArray",
    "InputStream" => "DataBytes",
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

fn build_rust(path:PathBuf, meta:DataObject, src:&str) {
  let _x = create_dir_all(&path);
  let id = &meta.get_string("id");
  let lib = &meta.get_string("lib");
  let ctl = &meta.get_string("ctl");
  let cmd = &meta.get_string("cmd");
  let data = meta.get_object("data");
  let import = &data.get_string("import");
  let returntype = &lookup_type(&data.get_string("returntype"));
  let params = &data.get_array("params");
  
  let path2 = &path.join(cmd.to_string()+".rs");
  let mut file = File::create(&path2).unwrap();

  let _x = file.write_all(b"use ndata::dataobject::*;\n");
  let _x = file.write_all(import.as_bytes());
  
  let n = params.len();
  let _x = file.write_all(b"\npub fn execute(");
  if n == 0 { let _x = file.write_all(b"_"); }
  let _x = file.write_all(b"o: DataObject) -> DataObject {\n");
  
  let mut index = 0;
  let mut invoke1 = "let ax = ".to_string()+cmd+"(";
  let mut invoke2 = "pub fn ".to_string()+cmd+"(";
  for v in params.objects() {
    let v = v.object();
    let name = &v.get_string("name");
    let t = &v.get_string("type");
    let typ = &lookup_type(t);
    let typ2;
    if typ == "DataObject" { typ2 = "object".to_string(); }
    else if typ == "DataArray" { typ2 = "array".to_string(); }
    else if typ == "DataBytes" { typ2 = "bytes".to_string(); }
    else if typ == "Data" { typ2 = "property".to_string(); }
    else { typ2 = typ.to_lowercase(); }
    let line = "let a".to_string() + &index.to_string() + " = o.get_" + &typ2 + "(\"" + name + "\");\n";
    let _x = file.write_all(line.as_bytes());
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
      let _x = file.write_all(b"array");
      let _x = file.write_all(b"(\"a\", ax);\n");
    }
    else if returntype == "DataBytes" {
      let _x = file.write_all(b"bytes");
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
  
  let m = "pub mod ".to_string()+lib+";";
  let path2 = &path2.parent().unwrap().parent().unwrap().join("lib.rs");
  if path2.exists() { build_mod(path2, &m); }
  
  let path2 = &path2.parent().unwrap().join("main.rs");
  if path2.exists() { build_mod(path2, &m); }
    
  let m = "    cmds.push((\"".to_string()+id+"\".to_string(), "+lib+"::"+ctl+"::"+cmd+"::execute, \"\".to_string()));";
  let mm = "use crate::".to_string()+lib+";";
  let path2 = &path2.parent().unwrap().join("cmdinit.rs");
  if path2.exists() {
    let file = File::open(&path2).unwrap();
    let lines = io::BufReader::new(file).lines();
    let mut part1 = Vec::<String>::new();
    let mut part2 = Vec::<String>::new();
    let mut a = true;
    let mut b = true;
    let mut c = true;
    let begin = "    cmds.clear();";
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
  }
  else {
      let mut file = File::create(&path2).unwrap();
      let _x = file.write_all(mm.as_bytes());
      let _x = file.write_all(b"\nuse flowlang::rustcmd::*;\n\n#[derive(Debug)]\npub struct Initializer {\n    pub data_ref: (&'static str, ((usize,usize),(usize,usize),(usize,usize))),\n    pub cmds: Vec<(String, Transform, String)>,\n}\n\n#[no_mangle]\npub fn mirror(state: &mut Initializer) {\n    flowlang::mirror(state.data_ref);\n    state.cmds.clear();\n");
      let _x = file.write_all(m.as_bytes());
      let _x = file.write_all(b"\n    for q in &state.cmds { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }\n}\n");
  }
}

fn build_python(pypath:PathBuf, path:PathBuf, meta:DataObject, src:&str) {
  let _x = create_dir_all(&pypath);
  let _x = create_dir_all(&path);
//  let id = meta["id"].as_str().unwrap();
//  let lib = meta["lib"].as_str().unwrap();
//  let ctl = meta["ctl"].as_str().unwrap();
  let ctlid = &meta.get_string("ctlid");
//  let cmd = meta["cmd"].as_str().unwrap();
  let data = meta.get_object("data");
  let import = data.get_string("import");
  let import = import.replace("\r", "\n");
//  let returntype = &lookup_type(data["returntype"].as_str().unwrap());
  let params = data.get_array("params");
  
  let path2 = &path.join(ctlid.to_string()+"-f.py");
  let mut file = File::create(&path2).unwrap();

  let _x = file.write_all(b"import sys\nsys.path.append(\"");
  let _x = file.write_all(pypath.canonicalize().unwrap().to_str().unwrap().as_bytes());
  let _x = file.write_all(b"\")\n\n");
  let _x = file.write_all(import.as_bytes());
  let _x = file.write_all(b"\ndef execute(args):\n  return ");
  let _x = file.write_all(ctlid.as_bytes());
  let _x = file.write_all(b"(**args)\n");
  let _x = file.write_all(b"\ndef ");
  let _x = file.write_all(ctlid.as_bytes());
  let _x = file.write_all(b"(");
  
  let mut invoke = "".to_string();
  for param in params.objects() {
    let param = param.object();
    if invoke != "" { invoke += ", "; }
    invoke += &param.get_string("name");
  }
  let _x = file.write_all(invoke.as_bytes());
  let _x = file.write_all(b"):\n");
  
  let src = indent(src.to_string());
  let _x = file.write_all(src.as_bytes());
  let _x = file.write_all(b"\n");
}

fn indent(src:String) -> String {
  let mut s = "".to_string();
  let mut lines = BufReader::new(src.as_bytes()).lines();
  while let Some(line) = lines.next() {
    s += "  ";
    s += &line.unwrap();
    s += "\n";
  }
  s
}
