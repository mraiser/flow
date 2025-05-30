fn map_type_to_schema(t: &str) -> &str {
  match t {
    "String" => "string",
    "Integer" | "Float" | "i32" | "i64" | "u32" | "u64" | "usize" | "f32" | "f64" | "DInt" | "DFloat" => "number",
    "Boolean" | "bool" | "DBool" => "boolean",
    "DataObject" | "JSONObject" | "DObject" => "object",
    "DataArray" | "JSONArray" | "DArray" => "array",
    "DNull" => "null",
    _ => "object",
  }
}

//appserver::init_globals();

// early-exit container
let mut out = DataObject::new();

// basic sanity on tool path  lib.control.command
let parts: Vec<&str> = tool.split('-').collect();
if parts.len() != 3 {
  return out;
}
let (lib, control, command) = (parts[0], parts[1], parts[2]);

// look up control meta-data
let store = DataStore::new();
if !store.exists(lib, "controls"){
  eprintln!("lib {} does not have controls", lib);
  return out;
}
let controls = store
.get_data(lib, "controls")
.get_object("data")
.get_array("list");

let mut control_id = String::new();
for i in 0..controls.len() {
    let c = controls.get_object(i);
    if c.get_string("name") == control {
        control_id = c.get_string("id");
        break;
    }
}

if control_id.is_empty() {
  eprintln!("NO CONTROL ID. weird.");
  return out;
}

// grab concrete command description
if !store.exists(lib, &control_id){
  eprintln!("lib {} does not have control {}", lib, control_id);
  return out;
}
let cmds = store
.get_data(lib, &control_id)
.get_object("data")
.get_array("cmd");

let mut cmd_id = String::new();
for i in 0..cmds.len() {
    let c = cmds.get_object(i);
    if c.get_string("name") == command {
        cmd_id = c.get_string("id");
        break;
    }
}

if cmd_id.is_empty() {
  eprintln!("NO COMMAND ID. weird.");
  return out;
}

if !store.exists(lib, &cmd_id){
  eprintln!("lib {} does not have command {}", lib, cmd_id);
  return out;
}

let data_ptr = store.get_data(lib, &cmd_id).get_object("data");
let typ = data_ptr.get_string("type");
let data_ptr = data_ptr.get_string(&typ);
if !store.exists(lib, &data_ptr){
  eprintln!("lib {} does not have data {}", lib, data_ptr);
  return out;
}
let real_data = store
.get_data(lib, &data_ptr)
.get_object("data");

// ── build JSON-Schema for inputs ──────────────────────────────────────────
let mut schema = DataObject::new();
schema.put_string("title", &format!("{}Arguments", command));
schema.put_string("type", "object");

let mut properties = DataObject::new();
let mut required = DataArray::new();
let mut input_descriptions: Vec<(String, String, String)> = Vec::new();

if real_data.has("params") {
  let params = real_data.get_array("params");
  for i in 0..params.len() {
    let p = params.get_object(i);
    let name = p.get_string("name");
    let typ  = p.get_string("type");
    let desc = if p.has("desc") { p.get_string("desc") } else { String::new() };

    // prop <-- one per argument
    let mut prop = DataObject::new();
    prop.put_string("type", map_type_to_schema(&typ));
    if !desc.is_empty() {
      prop.put_string("description", &desc);
    }
    properties.put_object(&name, prop);

    // required list (treat as optional if field "optional" == true)
    let optional = if p.has("optional") {
      p.get_bool("optional")
    } else {
      false
    };
    if !optional {
      required.push_string(&name);
    }

    input_descriptions.push((name, typ, desc));
  }
}
schema.put_object("properties", properties);
if required.len() > 0 {
  schema.put_array("required", required);
}

out.put_object("inputSchema", schema);

// ── outputs (unchanged) ───────────────────────────────────────────────────
if real_data.has("returntype") {
  let mut outputs = DataObject::new();
  outputs.put_string(
    "result",
    map_type_to_schema(&real_data.get_string("returntype")),
  );
  out.put_object("outputs", outputs);
}

// ── description & summary ────────────────────────────────────────────────
let mut description = if real_data.has("desc") {
    real_data.get_string("desc").trim().to_string()
} else {
    String::new()
};
if description != "" && !input_descriptions.is_empty() {
  description.push_str("\n\nInputs:\n");
  for (name, typ, desc) in &input_descriptions {
    description.push_str(&format!(
      "- `{}` (*{}*): {}\n",
      name,
      map_type_to_schema(typ),
      desc
    ));
  }
}
let summary = description.lines().next().unwrap_or("").trim();

out.put_string("description", &description);
out.put_string("summary", summary);
out.put_string("name", &tool);
out.put_array("tags", DataArray::new());
out.put_array("examples", DataArray::new());

out