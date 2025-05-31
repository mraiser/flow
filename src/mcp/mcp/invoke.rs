use ndata::dataobject::DataObject;
use crate::command::Command;
use ndata::dataarray::DataArray;
//use crate::appserver;

pub fn execute(o: DataObject) -> DataObject {
  let arg_0: DataObject = o.get_object("data");
  let ax = invoke(arg_0);
  let mut result_obj = DataObject::new();
  result_obj.put_object("a", ax);
  result_obj
}

pub fn invoke(data: DataObject) -> DataObject {
    // ── normalise param names ────────────────────────────────────────────────
    let tool      = data.get_string("name");          // spec key
    let arguments = data.get_object("arguments");     // spec key

    // guard: badly-formed name (lib.control.command)
    let parts: Vec<&str> = tool.split('-').collect();
    if parts.len() != 3 {
        return make_error(format!("Invalid tool name '{}'", tool));
    }
    let (lib, control, command) = (parts[0], parts[1], parts[2]);

    // ── dispatch to Flowlang command impl ────────────────────────────────────
    //appserver::init_globals();
    let cmd = Command::lookup(lib, control, command);
    let result = cmd.execute(arguments);

    match result {
        Ok(v)  => wrap_value(v),
        Err(e) => make_error(format!("{:?}", e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// helpers
// ─────────────────────────────────────────────────────────────────────────────

fn wrap_value(v: DataObject) -> DataObject {
    let v = v.get_property("a");

    let mut text = DataObject::new();
    text.put_string("type", "text");
    text.put_string("text", &ndata::Data::as_string(v));

    let mut content_arr = DataArray::new();
    content_arr.push_object(text);

    let mut out = DataObject::new();
    out.put_array("content", content_arr);
    out
}

fn make_error(msg: String) -> DataObject {
    let mut text = DataObject::new();
    text.put_string("type", "text");
    text.put_string("text", &msg);

    let mut content_arr = DataArray::new();
    content_arr.push_object(text);

    let mut out = DataObject::new();
    out.put_array("content", content_arr);
    out.put_boolean("isError", true);
    out
}
