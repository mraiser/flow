use ndata::dataobject::DataObject;
use crate::mcp::mcp::invoke::invoke;
use crate::mcp::mcp::initialize::initialize;
use ndata::data::Data::DNull;
use ndata::data::Data;
use crate::mcp::mcp::list_tools::list_tools;
use crate::mcp::mcp::list_prompts::list_prompts;
use crate::mcp::mcp::list_resources::list_resources;

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
