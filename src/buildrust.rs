use std::path::Path;
use std::path::PathBuf;
use std::fs::read_dir;
use std::fs::read_to_string;
use ndata::dataobject::DataObject;
use crate::DataStore;
use ndata::dataarray::DataArray;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::collections::HashMap;
use std::fs::create_dir_all;

const CMD_MOD_LINE:&str = "pub fn cmdinit(cmds: &mut Vec<(String, flowlang::rustcmd::Transform, String)>) {";
//const MYCRATE:&str = env!("CARGO_CRATE_NAME");

pub fn build_all() -> bool {
  let mut b = false;
  let libs = read_dir("data").unwrap();
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
            if build(&lib, &ctl, &cmd, &root) { b = true; }
          }
        }
      }
    }
  }
  
  if b {
    let meta = store.lib_info(&lib);
    if meta.has("cargo") {
      let cargo = meta.get_object("cargo");
      let filename = root.join("Cargo.toml");
      let file = File::open(&filename).unwrap();
      let mut indices = [0,0];
      let mut i = 0;
      let mut c = -1;
      let mut vec = Vec::new();
      let mut features = HashMap::new();
      let mut dependencies = HashMap::new();
      let lines = BufReader::new(file).lines();
      for line in lines {
        let line = line.unwrap();
        vec.push(line.to_owned());
        if line.starts_with("[") {
            if line == "[features]" {
              indices[0] = i+1;
              c = 0;
            }
            else if line == "[dependencies]" {
              indices[1] = i+1;
              c = 1;
            }
            else { c = -1; }
        }
        else if c != -1 {
          let off = line.chars().position(|c| c == '=');
          if off.is_some() {
            let off = off.unwrap();
            let k = line[..off].trim().to_owned();
            let v = line[off+1..].trim().to_owned();
            if c == 0 { features.insert(k,v); }
            else if c == 1 { dependencies.insert(k,v); }
          }  
        }
        
        i += 1;
      }
      
      let mut rewrite = false;
      
      if cargo.has("features") {
        let newf = cargo.get_object("features");
        for (k,v) in newf.objects() {
          let v = v.string();
          let newv = k.to_string() + " = " + &v;
          if !features.contains_key(&k) {
            vec.insert(indices[0], newv);
            rewrite = true;
          }
          else if features.get(&k).unwrap() != &v {
            println!("WARNING: Feature does not match existing: {}", newv);
            println!("OLD VALUE: {}", features.get(&k).unwrap());
            let mut vi = indices[0];
            while vi < vec.len() {
              if vec[vi].starts_with(&k) {
                vec[vi] = newv;
                rewrite = true;
                break;
              }
              vi += 1;
            }
          }
        }
      }
      
      if cargo.has("dependencies") {
        let newd = cargo.get_object("dependencies");
        for (k,v) in newd.objects() {
          let v = v.string();
          let newv = k.to_string() + " = " + &v;
          if !dependencies.contains_key(&k) {
            vec.insert(indices[1], newv);
            rewrite = true;
          }
          else if dependencies.get(&k).unwrap() != &v {
            println!("WARNING: Dependency does not match existing: {}", newv);
            println!("OLD VALUE: {}", dependencies.get(&k).unwrap());
            let mut vi = indices[1];
            while vi < vec.len() {
              if vec[vi].starts_with(&k) {
                vec[vi] = newv;
                rewrite = true;
                break;
              }
              vi += 1;
            }
          }
        }
      }
      
      if rewrite {
        println!("Rewriting {}", filename.display());
        let mut file = File::create(&filename).unwrap();
        for line in vec {
          let line = line + "\n";
          let _x = file.write_all(line.as_bytes());
        }
      }
    }
  }
  
  b
}

pub fn build(lib:&str, ctl:&str, cmd:&str, root:&Path) -> bool {
  //println!("BUILDING lib:{} ctl:{} root:{}", lib, cmd, root.display());
  let mut b = false;
  let store = DataStore::new();
  let id = &store.lookup_cmd_id(lib, ctl, cmd);
  //println!("ID {}", id);
  
  if store.exists(lib, id) {
    let meta = store.get_data(lib, id);
    let data = meta.get_object("data");
    let typ = data.get_string("type");
    //println!("TYPE {}", typ);
    
    let path = root.join("src").join(lib).join(ctl);
    if !path.exists() { let _x = std::fs::create_dir_all(path.clone()); }
      
    if typ == "rust" {
      let id = &data.get_string("rust");
      let mut meta = store.get_data(lib, id);
      
      let pathx = store.get_data_file(lib, &(id.to_owned()+".rs"));
      let src = store.read_file(pathx);
      
      meta.put_string("lib", lib);
      meta.put_string("ctl", ctl);
      meta.put_string("cmd", cmd);
      
      //println!("Building Rust: {}:{}:{}", lib, ctl, cmd);
      b |= build_rust(path.clone(), meta, &src);
      build_mod(path.clone(), &lib, &ctl, &cmd, &id);
    }
    else if typ == "python" {
      let cid = &data.get_string("python");
      let mut meta = store.get_data(lib, cid);
      
      let pathx = store.get_data_file(lib, &(cid.to_owned()+".python"));
      let src = store.read_file(pathx);
      
      meta.put_string("lib", lib);
      meta.put_string("ctl", ctl);
      meta.put_string("cmd", cmd);
      
      //println!("Building Python: {}:{}:{}", lib, ctl, cmd);
      build_python(path, meta, &src);
    }
  }  
  // FIXME - JS/Java/Flow
  
  b
}

fn file_index_of(path2:&PathBuf, m:&str) -> i64 {
  let file = File::open(&path2).unwrap();
  let lines = BufReader::new(file).lines();
  let mut i = 0;
  for line in lines {
    if let Ok(ip) = line {
      if ip == m {
        return i;
      }
    }
    i += 1;
  }
  -1
}

fn file_insert(path2:&PathBuf, m:&str, n:i64) {
  let mut news = "".to_string();
  let file = File::open(&path2).unwrap();
  let lines = BufReader::new(file).lines();
  let mut i = 0;
  for line in lines {
    if let Ok(ip) = line {
      news += &ip;
      news += "\n";
    }
    if i == n {
      news += m;
    }
    i += 1;
  }
  std::fs::write(path2.clone(), news).expect("Unable to write file");
}

fn file_remove(path2:&PathBuf, n:i64) {
  //println!("xxx");
  let mut news = "".to_string();
  let file = File::open(&path2).unwrap();
  let lines = BufReader::new(file).lines();
  let mut i = 0;
  for line in lines {
    if i != n {
        if let Ok(ip) = line {
          news += &ip;
          news += "\n";
        }
    }
    i += 1;
  }
  std::fs::write(path2.clone(), news).expect("Unable to write file");
}

fn build_mod(path:PathBuf, lib:&str, ctl:&str, cmd:&str, id:&str) {
    let m1 = "pub mod ".to_string()+cmd+";";
    let m2 = "    cmds.push((\"".to_string()+&id+"\".to_string(), "+(&cmd)+"::execute, \"\".to_string()));";
//    let m2 = "    ".to_string()+cratename+"::rustcmd::RustCmd::add(\""+&id+"\".to_string(), "+(&cmd)+"::execute, \"\".to_string());";
    let modfile = path.join("mod.rs");
    build_mod_file(modfile, m1, m2);

    let path = path.parent().unwrap().to_path_buf();
    let m1 = "pub mod ".to_string()+ctl+";";
    let m2 = "    ".to_string()+&ctl+"::cmdinit(cmds);";
    let modfile = path.join("mod.rs");
    build_mod_file(modfile, m1, m2.clone());

    let path = path.parent().unwrap().to_path_buf();
    let y = path.join("cmdinit.rs");
    if y.exists(){
        let m2 = "    cmds.push((\"".to_string()+&id+"\".to_string(), "+(&lib)+"::"+(&ctl)+"::"+(&cmd)+"::execute, \"\".to_string()));";
        let x = file_index_of(&y, &m2);
        if x != -1 {
            file_remove(&y, x);
        }
        
        let m2 = "    cmds.clear();";
        let x = file_index_of(&y, &m2);
        if x != -1 {
            file_remove(&y, x);
        }
    }
    
    let m1 = "use crate::".to_string()+lib+";";
    let m2 = "    ".to_string()+&lib+"::cmdinit(cmds);";
    build_mod_file(y, m1, m2);
    
    let m1 = "pub mod ".to_string()+lib+";";
    let path2 = path.join("lib.rs");
    if path2.exists() { 
        let x = file_index_of(&path2, &m1);
        if x == -1 {
            let s = m1.clone() + "\n";
            file_insert(&path2, &s, 0);
        }
    } // FIXME - else what?

    let path2 = path.join("main.rs");
    if path2.exists() { 
        let x = file_index_of(&path2, &m1);
        if x == -1 {
            let s = m1.clone() + "\n";
            file_insert(&path2, &s, 0);
        }
    } // FIXME - else what?
}

fn build_mod_file(modfile:PathBuf, m1:String, m2:String) {
    let cratename;
//    if MYCRATE == "flow" || MYCRATE == "flowlang" { cratename = "crate"; }
//    else { 
      cratename = "flowlang"; 
//    }
    
    
    // Step 1 - make sure the file exists
    if !modfile.exists() {
        let s = "\n".to_string()+CMD_MOD_LINE+"\n}";
        std::fs::write(modfile.clone(), s).expect("Unable to write file");
    }
    
    // step 2 - make sure the modline exists (add it)
    let y = file_index_of(&modfile, &m1);
    if y == -1 {
        let s = m1+"\n"+&read_to_string(&modfile).unwrap();
        std::fs::write(modfile.clone(), s).expect("Unable to write file");
    }
    
    // step 3 - make sure the cmdinit exists (add it)
    let mut x = file_index_of(&modfile, CMD_MOD_LINE);
    if x == -1 { 
    
        // use crate::RustCmd;
    
    
        let s = "\nuse ".to_string()+cratename+"::rustcmd::*;\n"+CMD_MOD_LINE+"\n}";
        let mut file = OpenOptions::new()
            .append(true)
            .open(&modfile)
            .unwrap();
        if let Err(e) = writeln!(file, "{}", s) { eprintln!("Couldn't write to file: {}", e); }    
        x = file_index_of(&modfile, CMD_MOD_LINE); // FIXME - sloppy
    }
    
    // Step 4 - make sure the cmds.push or RustCmd::add exists (add it)
    let z = file_index_of(&modfile, &m2);
    if z == -1 {
        let s = m2 + "\n";
        file_insert(&modfile, &s, x);
    }
}

fn build_rust(path:PathBuf, meta:DataObject, src:&str) -> bool {
    let mut b = false;

    let new_src = build_rust_source(meta.clone(), src);

    let cmd = meta.get_string("cmd");
    let rustfile = path.join(cmd.to_string()+".rs");
    let old_src;
    if rustfile.exists(){
        old_src = read_to_string(&rustfile).unwrap();
    }
    else {
        old_src = "-99 not valid".to_string();
    }
    
    if old_src != new_src { // FIXME - what if compile files and we try again?
        b = true;
    }
    
    if b {
      std::fs::write(rustfile, new_src).expect("Unable to write file");
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

fn build_rust_source(meta:DataObject, code:&str) -> String {
    let data = meta.get_object("data");
    let cmd = meta.get_string("cmd");
    
    let import = &data.get_string("import");
    let returntype = &lookup_type(&data.get_string("returntype"));
    let params = &data.get_array("params");
    
    let mut src = "use ndata::dataobject::*;\n".to_string();
    src += import;
  
    let n = params.len();
    src += "\npub fn execute(";
    if n == 0 { src += "_"; }
    src += "o: DataObject) -> DataObject {\n";
    
    let (invoke0, invoke1, invoke2) = build_rust_invoke(&cmd, params.clone(), &returntype);
    let retstr = build_rust_return(&returntype);
    
    src += &invoke0;
    src += &invoke1;
    src += "let mut o = DataObject::new();\n";
    src += &retstr;
    
    src += "o\n";
    src += "}\n\n";
    src += &invoke2;
    src += &code;
    src += "\n}\n\n";
    
    src
}

fn build_rust_return(returntype:&str) -> String {
  let mut s = "".to_string();
  if returntype == "Data" {
    s += "o.set_property(\"a\", ax);\n";
  }
  else {
    s += "o.put_";
    if returntype == "String" {
      s += "string";
      s += "(\"a\", &ax);\n";
    }
    else if returntype == "f64" {
      s += "float";
      s += "(\"a\", ax);\n";
    }
    else if returntype == "i64" {
      s += "int";
      s += "(\"a\", ax);\n";
    }
    else if returntype == "bool" {
      s += "boolean";
      s += "(\"a\", ax);\n";
    }
    else if returntype == "DataObject" {
      s += "object";
      s += "(\"a\", ax);\n";
    }
    else if returntype == "DataArray" {
      s += "array";
      s += "(\"a\", ax);\n";
    }
    else if returntype == "DataBytes" {
      s += "bytes";
      s += "(\"a\", ax);\n";
    }
    else {
      s += &returntype;
      s += "(\"a\", ax);\n";
    }
  }
  s
}

fn build_rust_invoke(cmd: &str, params: DataArray, returntype: &str) -> (String, String, String) {
    let mut index = 0;
    let mut invoke0 = "".to_string();
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
        else if typ == "bool" { typ2 = "boolean".to_string(); }
        else if typ == "i64" { typ2 = "int".to_string(); }
        else if typ == "f64" { typ2 = "float".to_string(); }
        else { typ2 = typ.to_lowercase(); }
        let line = "let a".to_string() + &index.to_string() + " = o.get_" + &typ2 + "(\"" + name + "\");\n";
        invoke0 += &line;
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
    
    (invoke0, invoke1, invoke2)
}

fn build_python(path:PathBuf, meta:DataObject, src:&str) {
    let cmd = meta.get_string("cmd");
    let pyfile = path.join(cmd.to_string()+".py");
    
    let data = meta.get_object("data");
    let params = data.get_array("params");
    let import = data.get_string("import");
    let import = import.replace("\r", "\n");
    
    let mut invoke = "".to_string();
    let mut invoke2 = "".to_string();
    for param in params.objects() {
        let param = param.object();
        if invoke != "" { invoke += ", "; }
        invoke += &param.get_string("name");
        if invoke2 != "" { invoke2 += ", "; }
        invoke2 += "args['";
        invoke2 += &param.get_string("name");
        invoke2 += "']";
    }
    
    let mut new_src = import;
    new_src += "def execute(args):\n  return ";
    new_src += &cmd;
    new_src += "(";
    new_src += &invoke2;
    new_src += ")\n\ndef ";
    new_src += &cmd;
    new_src += "(";
    new_src += &invoke;
    new_src += "):\n";
    
    let mut lines = BufReader::new(src.as_bytes()).lines();
    while let Some(line) = lines.next() {
        new_src += "  ";
        new_src += &line.unwrap();
        new_src += "\n";
    }
    
    new_src += "\nif __name__ == \"__main__\":\n    print(json.dumps(execute(json.loads(sys.argv[1]))))";
    
    std::fs::write(pyfile, new_src).expect("Unable to write file");
}

pub fn rebuild_rust_api() {
    let store = DataStore::new();
    let mut apistr = "pub const fn new() -> api {\n  api {\n".to_string();
    let mut libstr = "pub struct api {\n".to_string();
    let mut ctlstr = "".to_string();
    let mut cmdstr = "".to_string();
    let mut impstr = "".to_string();

    fn lookup_dtype(ptype:&str) -> &str {
      match ptype.as_ref() {
        "FLAT" => "DataObject",
        "JSONObject" => "DataObject",
        "JSONArray" => "DataArray",
        "InputStream" => "DataBytes",
        "float" => "f64",
        "Integer" => "i64",
        "Boolean" => "bool",
        "Any" => "Data",
        "NULL" => "DNull",
        _ => "String"
      }
    }

    fn lookup_ntype(ptype:&str) -> &str {
      match ptype.as_ref() {
        "FLAT" => "DataObject",
        "JSONObject" => "object",
        "JSONArray" => "array",
        "InputStream" => "bytes",
        "float" => "float",
        "Integer" => "int",
        "Boolean" => "boolean",
        "Any" => "property",
        "NULL" => "null",
        _ => "string"
      }
    }

    let libs = read_dir("data").unwrap();
    for db in libs {
      let lib = db.unwrap().file_name().into_string().unwrap();
      let root = store.get_lib_root(&lib);
      if store.exists(&lib, "controls") {
        apistr = apistr + "    "+(&lib) + ": " + (&lib) + " {\n";
        libstr = libstr + "  pub " + (&lib) + ": " + (&lib) + ",\n";
        ctlstr = ctlstr + "pub struct " + (&lib) + " {\n";
        let controls = store.get_data(&lib, "controls");
        let list = controls.get_object("data").get_array("list");
        for control in list.objects() {
          let control = control.object();
          let ctl = control.get_string("name");
          let id = control.get_string("id");
          if store.exists(&lib, &id) {
            apistr = apistr + "      "+(&ctl) + ": " + (&lib) + "_" + (&ctl) + " {},\n";
            ctlstr = ctlstr + "  pub " + (&ctl) + ": " + (&lib) + "_" + (&ctl) + ",\n";
            cmdstr = cmdstr + "pub struct " + (&lib) + "_" + (&ctl) + " {}\n";
            let ctldata = store.get_data(&lib, &id);
            let d = ctldata.get_object("data");
            if d.has("cmd") {
              let cmdlist = d.get_array("cmd");
              if cmdlist.len() > 0 {
                impstr = impstr + "impl " + (&lib) + "_" + (&ctl) + " {\n";
                for command in cmdlist.objects() {
                  let command = command.object();
                  let cmd = command.get_string("name");
                  let id = command.get_string("id");
                  if store.exists(&lib, &id) {
                    let meta = store.get_data(&lib, &id);
                    let data = meta.get_object("data");
                    let typ = data.get_string("type");
                    if typ == "rust" {
                      impstr = impstr + "  pub fn " + (&cmd) + "(&self";
                      let mut pstr = "".to_string();
                      let id = &data.get_string("rust");
                      let meta = store.get_data(&lib, &id).get_object("data");
                      let params = meta.get_array("params");
                      for param in params.objects(){
                        let param = param.object();
                        let pname = param.get_string("name");
                        let ptype = param.get_string("type");
                        let dtype = lookup_dtype(&ptype);
                        let ntype = lookup_ntype(&ptype);
                        impstr = impstr + ", " + (&pname) + ":" + dtype;
                        let q = match(ntype.as_ref()) {
                          "string" => "&",
                          _ => ""
                        };
                        let x = match(ntype.as_ref()) {
                          "property" => "set",
                          _ => "put"
                        };
                        pstr = pstr + "    d." + x + "_" + (&ntype) + "(\"" + (&pname) + "\", " + q + (&pname) + ");\n";
                      }
                      
                      
                      
                      
                      let rtype = meta.get_string("returntype");
                      let ntype = lookup_ntype(&rtype);
                      let rtype = lookup_dtype(&rtype);
                      impstr = impstr + ") -> " + rtype + " {\n    let mut d = DataObject::new();\n" + &pstr;
                      impstr = impstr + "    RustCmd::new(\"" + (&id) + "\").execute(d).unwrap().get_" + (&ntype) + "(\"a\")\n  }\n";
                    }
                  }
                }
                impstr += "}\n";
              }
            }
          }
        }
        apistr += "    },\n";
        ctlstr += "}\n";
      }
    }
    apistr += "  }\n}\n";
    libstr += "}";

    let usestr = r#"use ndata::dataobject::DataObject;
use ndata::dataarray::DataArray;
use ndata::databytes::DataBytes;
use ndata::data::Data;
use flowlang::rustcmd::RustCmd;
"#;

    //println!("{}", usestr);
    //println!("{}", cmdstr);
    //println!("{}", ctlstr);
    //println!("{}", libstr);
    //println!("{}", apistr);
    //println!("{}", impstr);
    
    let s = usestr.to_string()
        + &cmdstr
        + &ctlstr
        + &libstr
        + &apistr
        + &impstr;
    
    let path = store.root.parent().unwrap().join("cmd").join("src").join("api.rs");
    let _x = std::fs::write(&path, &s);
}
