let cmd = Command::new(&lib, &id);
let ret = cmd.return_type.to_owned();
let o = cmd.execute(params).unwrap();

if ret == "FLAT" { return o; }

let key;
if o.has("data") { key = "data".to_string(); }
else if o.has("msg") { key = "msg".to_string(); }
else if o.has("a") { key = "a".to_string(); }
else {
  let params = o.duplicate().keys();
  if params.len() == 0 { 
    return o; 
  }
  key = params[0].to_owned();
}
let val = o.get_property(&key);
let mut o = DataObject::new();
if ret == "String" { o.set_property("msg", val); }
else { o.set_property("data", val); }
o.put_str("status", "ok");
o
