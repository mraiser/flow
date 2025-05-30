appserver::init_globals();

let mut out = DataObject::new();
let mut tools = DataArray::new();
let store = DataStore::new();

//let libs = crate::API.app.app.libs();
let o = DataStore::globals().get_object("system").get_object("libraries");
let mut libs = DataArray::new();
for (_k,v) in o.objects(){ libs.push_property(v); }

for i in 0..libs.len() {
  let lib_obj = libs.get_object(i);
  let lib = lib_obj.get_string("id");
  
//if lib != "app" { continue; }

  if !store.exists(&lib, "controls") {
    continue;
  }

  let controls = store.get_data(&lib, "controls");

  if !controls.has("data") {
    continue;
  }

  let list_obj = controls.get_object("data");

  if !list_obj.has("list") {
    continue;
  }

  let list = list_obj.get_array("list");

  for j in 0..list.len() {
    let control_obj = list.get_object(j);
    let control_name = control_obj.get_string("name");
    let control_id = control_obj.get_string("id");

    if !store.exists(&lib, &control_id) {
      continue;
    }

    let control_data = store.get_data(&lib, &control_id);

    if !control_data.has("data") {
      continue;
    }

    let data_obj = control_data.get_object("data");

    if !data_obj.has("cmd") {
      continue;
    }

    let cmds = data_obj.get_array("cmd");
    for k in 0..cmds.len() {
      let cmd_obj = cmds.get_object(k);
      let cmd_name = cmd_obj.get_string("name");
      let fullname = format!("{}-{}-{}", lib, control_name, cmd_name);
      let desc = describe(fullname);
      
      
      
      
      if desc.has("description"){
        if desc.get_string("description").trim() != "" {
          tools.push_object(desc);
        }
      }
      
      
      
      
    }
  }
}

out.put_array("tools", tools);
out