use ::flowlang::*;

use std::env;
use std::io;
use std::io::BufRead;
use ndata::dataobject::DataObject;

use command::Command as Command;

/*
pub fn init(dir:&str) -> (&str, NDataConfig) {
  let cfg = DataStore::init(dir);
  let mut v = Vec::new();
  cmdinit(&mut v);
  for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
  cfg
}

pub fn mirror(q:(&str, NDataConfig)) {
  DataStore::mirror(q);
  let mut v = Vec::new();
  cmdinit(&mut v);
  for q in &v { RustCmd::add(q.0.to_owned(), q.1, q.2.to_owned()); }
}
*/

pub fn main() {
  init("data");
  
  env::set_var("RUST_BACKTRACE", "1");
  {
    let params: Vec<String> = env::args().collect();
    // flow pytest [pytest args]: run pytest inside the embedded Python runtime.
    if params.len() > 1 && params[1] == "pytest" {
      #[cfg(feature = "python_runtime")]
      std::process::exit(::flowlang::pyenv::run_pytest(params[2..].to_vec()));
      #[cfg(not(feature = "python_runtime"))]
      {
        eprintln!("flow pytest needs the embedded Python runtime: build with --features=python_runtime");
        std::process::exit(2);
      }
    }
    // flow jstest [--no-check] [files or directories]: run *.test.js and
    // *.test.ts in the JavaScript engine, type-checked first.
    if params.len() > 1 && params[1] == "jstest" {
      #[cfg(feature = "javascript_runtime")]
      std::process::exit(::flowlang::jscmd::run_tests(params[2..].to_vec()));
      #[cfg(not(feature = "javascript_runtime"))]
      {
        eprintln!("flow jstest needs the JavaScript runtime: build with --features=javascript_runtime");
        std::process::exit(2);
      }
    }
    if params.len() < 4 {
      eprintln!("usage: flow <lib> <ctl> <cmd>  (JSON arguments on stdin)\n       flow pytest [pytest arguments]\n       flow jstest [--no-check] [files or directories]");
      std::process::exit(2);
    }
    let lib = &params[1];
    let ctl = &params[2];
    let cmd = &params[3];

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut s = "".to_string();
    while let Some(line) = lines.next() {
      s = s + &line.unwrap();
    }
    
    // A failure prints {"status": "err", "msg": ...} (for a failed flow, with "flow_error":
    // what failed and where) and exits with status 1.
    let args = if s.trim().is_empty() { DataObject::new() } else {
      match DataObject::try_from_string(&s) {
        Ok(o) => o,
        Err(e) => fail(format!("the arguments on stdin are not a JSON object: {}", e)),
      }
    };
    let target = match callapi::resolve_command(lib, ctl, cmd) {
      Ok(t) => t,
      Err(e) => fail(e),
    };
    let cmd = Command::new(&target.lib, &target.id);
    match cmd.execute(args) {
      Ok(res) => println!("{}", res.to_string()),
      Err(e) => { println!("{}", e.to_envelope().to_string()); std::process::exit(1); }
    }
  }
}

fn fail(msg: String) -> ! {
  let mut o = DataObject::new();
  o.put_string("status", "err");
  o.put_string("msg", &msg);
  println!("{}", o.to_string());
  std::process::exit(1);
}
