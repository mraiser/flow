    println!("[MCP] Incoming request: {}", data.to_string());

    let id = if data.has("id") { data.get_property("id") } else { DNull };

    if !data.has("jsonrpc") || !data.has("method") {
        let err = error_response(id, -32600_i64, "Invalid Request");
        println!("[MCP] Responding with error: {}", err.to_string());
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
            println!("[MCP] Responding with error: {}", err.to_string());
            return err;
        }
    };

    let mut response = DataObject::new();
    response.put_string("jsonrpc", "2.0");
    response.set_property("id", id);
    response.put_object("result", result);

    println!("[MCP] Response: {}", response.to_string());
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