use ndata::dataobject::DataObject;
use crate::mcp::mcp::invoke::invoke;
use crate::mcp::mcp::initialize::initialize;
use ndata::data::Data::DNull;
use ndata::data::Data;
use crate::mcp::mcp::list_tools::list_tools;
use crate::mcp::mcp::list_prompts::list_prompts;
use crate::mcp::mcp::list_resources::list_resources;
use std::io::{self, BufRead, Write};

pub fn execute(o: DataObject) -> DataObject {
  let arg_0: DataObject = o.get_object("data");
  let ax = mcp(arg_0);
  let mut result_obj = DataObject::new();
  result_obj.put_object("a", ax);
  result_obj
}

pub fn mcp(data: DataObject) -> DataObject {
    eprintln!("[MCP] Incoming request: {}", data.to_string());

    let id = if data.has("id") { data.get_property("id") } else { DNull };

    if !data.has("jsonrpc") || !data.has("method") {
        let err = error_response(id, -32600_i64, "Invalid Request");
        eprintln!("[MCP] Responding with error: {}", err.to_string());
        return err;
    }

    let method = data.get_string("method");
    let params = if data.has("params") { data.get_object("params") } else { DataObject::new() };

    let result = match method.as_str() {
        "tools/call" => invoke(params),
        "tools/list" => list_tools(),
        "prompts/list" => list_prompts(),
        "resources/list" => list_resources(),
        "initialize" => initialize(),
        _ => {
            let err = error_response(id.clone(), -32601_i64, "Method not found");
            eprintln!("[MCP] Responding with error: {}", err.to_string());
            return err;
        }
    };

    let mut response = DataObject::new();
    response.put_string("jsonrpc", "2.0");
    response.set_property("id", id);
    response.put_object("result", result);

    eprintln!("[MCP] Response: {}", response.to_string());
    response
}

pub fn run(){
    eprintln!("[flow] MCP flow started — expecting JSON-RPC over stdin/stdout");

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        match line {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }

                eprintln!("[flow] Incoming line: {}", line);

                match DataObject::try_from_string(&line) {
                    Ok(query) => {
                        if !query.has("id") || !query.get_property("id").is_number() {
                            eprintln!("[flow] NOT GONNA DO IT JACK: {}", query.to_string());
                            continue;
                        }

                        let response = {
                            #[cfg(feature = "gag")]
                            let _stdout_gag = Gag::stdout().unwrap();
                            mcp(query)
                        };

                        eprintln!("[flow] Outgoing response: {}", response.to_string());
                        writeln!(stdout_lock, "{}", response.to_string()).unwrap();
                        stdout_lock.flush().unwrap();
                    }
                    Err(err) => {
                        eprintln!("[flow] Error during request: {}", err);
                        let msg = format!("Internal error: {}", err);
                        let mut error = DataObject::new();
                        error.put_int("code", -32603);
                        error.put_string("message", &msg);
                        let mut fallback = DataObject::new();
                        fallback.put_string("jsonrpc","2.0");
                        fallback.put_null("id");
                        fallback.put_object("error", error);
                        writeln!(stdout_lock, "{}", fallback.to_string()).unwrap();
                        stdout_lock.flush().unwrap();
                    }
                }
            }
            Err(err) => {
                eprintln!("[flow] Error reading stdin: {}", err);
                break;
            }
        }
    }

    eprintln!("[flow] Stream closed. Exiting MCP flow.");
}

fn error_response(id: Data, code: i64, message: &str) -> DataObject {
  let mut error = DataObject::new();
  error.put_int("code", code);
  error.put_string("message", message);

  let mut response = DataObject::new();
  response.put_string("jsonrpc", "2.0");
  response.set_property("id", id);
  response.put_object("error", error);
  response
}
