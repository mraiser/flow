use ndata::dataobject::*;
use std::thread;
use std::panic;
use std::fs;
use std::path::Path;
use std::fs::create_dir_all;
use std::fs::File;
use ndata::dataarray::*;
use std::time::Duration;
use ndata::data::Data;
use state::Storage;
use std::sync::RwLock;
use std::sync::Once;

use crate::command::*;
use crate::datastore::*;

use crate::generated::flowlang::system::time::time;
use crate::generated::flowlang::system::unique_session_id::unique_session_id;
use crate::generated::flowlang::object::index_of::index_of;
use crate::generated::flowlang::file::read_properties::read_properties;
use crate::generated::flowlang::file::write_properties::write_properties;

// FIXME - The code in this file makes the assumption in several places that the process was launched from the root directory. That assumption should only be made once, in the event that no root directory is specified, by whatever initializes the flowlang DataStore.

pub type EventHook = fn(&str,&str,DataObject);

static START: Once = Once::new();
pub static EVENTHOOKS:Storage<RwLock<Vec<EventHook>>> = Storage::new();

pub fn run() {
  START.call_once(|| { EVENTHOOKS.set(RwLock::new(Vec::new())); });  

  let system = init_globals();
  
  // Start Timers
  thread::spawn(timer_loop);
  
  // Start Events
  thread::spawn(event_loop);
    
  // Check sessions
  let dur = Duration::from_millis(5000);
  let mut sessions = system.get_object("sessions");
  let sessiontimeoutmillis = system.get_object("config").get_i64("sessiontimeoutmillis");
  
  while system.get_bool("running") {
    let expired = time() - sessiontimeoutmillis; 
    for (k,v) in sessions.objects() {
      let v = v.object();
      let expire = v.get_i64("expire");
      if expire < expired {
        println!("Session expired {} {} {}", k, v.get_string("username"), v.get_object("user").get_string("displayname"));
        sessions.remove_property(&k);
        fire_event("app", "SESSION_EXPIRE", v);
      }
    }
    thread::sleep(dur);
  }
}

pub fn check_auth(lib:&str, id:&str, session_id:&str, write:bool) -> bool {
  let store = DataStore::new();
  let system = DataStore::globals().get_object("system");
  
  if !system.get_object("config").get_bool("security") { 
    return true; 
  }
  
  let libdata = system.get_object("libraries").get_object(lib);
  let libgroups = libdata.get_array("readers");
  
  let which;
  if write { which = "writers"; }
  else { which = "readers"; }
  
  let ogroups;
  if !store.get_data_file(lib, id).exists() {
    ogroups = libgroups.duplicate();
  }
  else {
    let data = store.get_data(lib, id);
    if data.has(which) { ogroups = data.get_array(which); }
    else { ogroups = DataArray::new(); }
  }
  
  let lg = Data::DArray(libgroups.data_ref);
  let og = Data::DArray(ogroups.data_ref);
    
  if index_of(lg.clone(), Data::DString("anonymous".to_string())) != -1 {
    if index_of(og.clone(), Data::DString("anonymous".to_string())) != -1 {
      return true;
    }
  }

  let sessions = system.get_object("sessions");
  let groups;
  if sessions.has(session_id) {
    let session = sessions.get_object(session_id);
    let user = session.get_object("user");
    groups = user.get_array("groups");
  }
  else { groups = DataArray::new(); }
  
  if index_of(Data::DArray(groups.data_ref), Data::DString("admin".to_string())) != -1 {
    return true;
  }
  
  for g in groups.objects() {
    if index_of(lg.clone(), g.clone()) != -1 {
      if index_of(og.clone(), g.clone()) != -1 {
        return true;
      }
    }
  }
    
  false
}

pub fn check_security(command:&Command, session_id:&str) -> bool {
//  println!("session id: {}", session_id);
  let system = DataStore::globals().get_object("system");
  
  if !system.get_object("config").get_bool("security") { 
    return true; 
  }
    
  let lib = system.get_object("libraries").get_object(&command.lib);
  
  let libgroups = lib.get_property("readers");
  let cmdgroups = &command.readers;
  if index_of(libgroups.clone(), Data::DString("anonymous".to_string())) != -1 {
    if cmdgroups.iter().position(|r| r == "anonymous").is_some() {
      return true;
    }
  }
  
  let sessions = system.get_object("sessions");
  let groups;
  if sessions.has(session_id) {
    let session = sessions.get_object(session_id);
    let user = session.get_object("user");
    groups = user.get_array("groups");
  }
  else { groups = DataArray::new(); }
  
  if index_of(Data::DArray(groups.data_ref), Data::DString("admin".to_string())) != -1 {
    return true;
  }
  
  for g in groups.objects() {
    if index_of(libgroups.clone(), g.clone()) != -1 {
      if cmdgroups.iter().position(|r| r == &(g.string())).is_some() {
        return true;
      }
    }
  }
    
  false
}

pub fn log_in(sessionid:&str, username:&str, password:&str) -> bool {
  let user = get_user(username);
  let mut e = DataObject::new();
  e.put_str("user", username);
  e.put_str("sessionid", sessionid);
  if user.is_some() {
    let user = user.unwrap();
    if user.get_string("password") == password {
      let system = DataStore::globals().get_object("system");
      let sessions = system.get_object("sessions");
      let mut session = sessions.get_object(sessionid);
      session.put_str("username", username);
      session.put_object("user", user);
      
      fire_event("security", "LOGIN", e);

      return true;
    }
  }

  fire_event("security", "LOGIN_FAIL", e);
  
  false
}

pub fn remove_timer(tid:&str) -> bool {
  let system = DataStore::globals().get_object("system");
  let mut timers = system.get_object("timers");
  if timers.has(tid) {
    timers.remove_property(tid);
    return true;
  }
  false
}

pub fn add_timer(tid:&str, mut tdata:DataObject) {
  let system = DataStore::globals().get_object("system");
  let mut timers = system.get_object("timers");    
  let start = tdata.get_i64("start");
  let start = time()+to_millis(start, tdata.get_string("startunit"));
  let interval = tdata.get_i64("interval");
  let interval = to_millis(interval, tdata.get_string("intervalunit"));
  tdata.put_i64("startmillis", start);
  tdata.put_i64("intervalmillis", interval);
  timers.put_object(&tid, tdata);
}

pub fn add_event_hook(hook:EventHook) {
  EVENTHOOKS.get().write().unwrap().push(hook);
}

pub fn remove_event_listener(id:&str) -> bool {
  let mut b = false;
  let system = DataStore::globals().get_object("system");
  let events = system.get_object("events");
  for (_k1, v1) in events.objects(){
    for (_k2, v2) in v1.object().objects(){
      for k3 in v2.object().keys(){
        if k3 == id { 
          v2.object().remove_property(&k3); 
          b = true;
        }
      }
    }
  }
  b
}

pub fn add_event_listener(id:&str, app:&str, event:&str, cmdlib:&str, cmdid:&str) {
  //println!("Adding event listener {}, {}, {}, {}, {}", id, app, event, cmdlib, cmdid);
  let system = DataStore::globals().get_object("system");
  let mut events = system.get_object("events");
  let mut bot;
  let mut list;
  if events.has(app) {
    bot = events.get_object(app);
  }
  else {
    bot = DataObject::new();
    events.put_object(app, bot.duplicate());
  }
  if bot.has(event) {
    list = bot.get_object(event);
  }
  else {
    list = DataObject::new();
    bot.put_object(event, list.duplicate());
  }
  let mut cmd = DataObject::new();
  cmd.put_str("lib", cmdlib);
  cmd.put_str("cmd", cmdid);
  list.put_object(id, cmd);
}

pub fn fire_event(app:&str, event:&str, data:DataObject) {
  let mut o = DataObject::new();
  o.put_str("app", &app);
  o.put_str("event", &event);
  o.put_object("data", data);
  DataStore::globals().get_object("system").get_array("fire").push_object(o);
}

pub fn event_loop() {
  let system = DataStore::globals().get_object("system");
  let mut events = system.get_object("events");
  let fire = system.get_array("fire");
  let dur = Duration::from_millis(100);
  while system.get_bool("running") {
    let mut b = true;
    for oprop in fire.objects(){
      let o = oprop.object();
      fire.remove_data(oprop);
      b = false;
      
      let app = o.get_string("app");
      let event = o.get_string("event");
      let data = o.get_object("data");
      
      if !events.has(&app) { events.put_object(&app, DataObject::new()); }
      let mut bot = events.get_object(&app);
      if !bot.has(&event) { bot.put_object(&event, DataObject::new()); }
      else {
        let list = bot.get_object(&event);
        for (_, e) in list.objects() {
          let e = e.object();
          let lib = e.get_string("lib");
          let id = e.get_string("cmd");
          let data = data.duplicate();
          thread::spawn(move || {
            let command = Command::new(&lib, &id);
            let  _ = command.execute(data);
          });
        }
      }
      
      for hook in EVENTHOOKS.get().write().unwrap().iter() {
        hook(&app, &event, o.duplicate());
      }
    }
        
    if b { thread::sleep(dur); }
  }
}

fn timer_loop() {
  let system = DataStore::globals().get_object("system");
  let dur = Duration::from_millis(1000);
  while system.get_bool("running") {
    let now = time();
    let mut timers = system.get_object("timers");
    for (id, timer) in timers.objects() {
      let mut timer = timer.object();
      let when = timer.get_i64("startmillis");
      if when <= now {
        timers.remove_property(&id);
        let cmdid = timer.get_string("cmd");
        let params = timer.get_object("params");
        let repeat = timer.get_bool("repeat");
        let db = timer.get_string("cmddb");
        let mut ts = timers.duplicate();
        thread::spawn(move || {
          let cmd = Command::new(&db, &cmdid);
          let _x = cmd.execute(params).unwrap();
          
          if repeat {
            let next = now + timer.get_i64("intervalmillis");
            timer.put_i64("startmillis", next);
            ts.put_object(&id, timer);
          }
        });
      }
    }
    
    thread::sleep(dur);
  }
}

pub fn load_library(j:&str) {
  let store = DataStore::new();
  let system = DataStore::globals().get_object("system");
  let path = store.root.join(j).join("meta.json");
  if path.exists() {
    let s = fs::read_to_string(&path).unwrap();
    let mut o2 = DataObject::from_string(&s);

    let mut readers = DataArray::new();
    let mut writers = DataArray::new();
    if o2.has("readers") { 
    for r in o2.get_array("readers").objects() { readers.push_str(&(r.string())); }
    }
    if o2.has("writers") { 
    for w in o2.get_array("writers").objects() { writers.push_str(&(w.string())); }
    }
    o2.put_array("readers", readers);
    o2.put_array("writers", writers);
    o2.put_str("id", j);

    let mut libraries = system.get_object("libraries");
    libraries.put_object(j, o2);
  }
}

pub fn get_user(username:&str) -> Option<DataObject> {
  let system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let users = system.get_object("users");
    if users.has(username) {
      return Some(users.get_object(username));
    }
  }
  None
}

pub fn delete_user(username:&str) -> bool{
  let system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let mut users = system.get_object("users");
    if users.has(&username) {
      users.remove_property(&username);
      let root = DataStore::new().root.parent().unwrap().join("users");
      let propfile = root.join(&(username.to_owned()+".properties"));
      let x = fs::remove_file(propfile);
      if x.is_ok() { return true; }
    }
  }
  false
}

pub fn set_user(username:&str, user:DataObject) {
  let system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let mut users = system.get_object("users");
    if !users.has(&username) { users.put_object(username, user.deep_copy()); }
    
    let mut user = user.deep_copy();
    let groups = user.get_array("groups");
    let mut s = "".to_string();
    for g in groups.objects() {
      let g = g.string();
      if s != "" { s += ","; }
      s += &g
    }
    user.put_str("groups", &s);
    let root = DataStore::new().root.parent().unwrap().join("users");
    let propfile = root.join(&(username.to_owned()+".properties"));
    write_properties(propfile.into_os_string().into_string().unwrap(), user);
  }
}

pub fn load_users() {
  let mut system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let mut users;
    let mut b = false;
    if system.has("users") { users = system.get_object("users"); }
    else {
      b = true;
      users = DataObject::new();
    }
    
    let root = DataStore::new().root.parent().unwrap().join("users");
    let propfile = root.join("admin.properties");
    if !propfile.exists() {
      let _x = create_dir_all(&root);
      let mut admin = DataObject::new();
      admin.put_str("displayname", "System Administrator");
      admin.put_str("groups", "admin");
      admin.put_str("password", &unique_session_id());
      write_properties(propfile.into_os_string().into_string().unwrap(), admin);
    }
    
    for file in fs::read_dir(&root).unwrap() {
      let file = file.unwrap();
      let name = file.file_name().into_string().unwrap();
      if name.ends_with(".properties") {
        let mut user = read_properties(file.path().into_os_string().into_string().unwrap());
        let id = &name[..name.len()-11];
        let groups = user.get_string("groups");
        let mut da = DataArray::new();
        for group in groups.split(",") { da.push_str(group); }
        user.put_array("groups", da);
        user.put_array("connections", DataArray::new());
        user.put_str("id", &id);
        
        if user.has("addresses"){
          let s = user.get_string("addresses");
          let da = DataArray::from_string(&s);
          user.put_array("addresses", da);
        }
        
        users.put_object(id, user);
      }
    }
    
    if b { system.put_object("users", users.duplicate()); }
  }
}

pub fn save_config(config: DataObject) {
  let mut config = config.deep_copy();
  if !config.has("security") { config.put_str("security", "on"); }
  else { 
    let b = config.get_bool("security");
    if b { config.put_str("security", "on"); } 
    else { config.put_str("security", "off"); } 
  }
  write_properties("config.properties".to_string(), config);
}

pub fn load_config() -> DataObject {
  println!("Loading appserver configuration");
  let mut b = false;
  let mut config;
  if Path::new("config.properties").exists() {
    config = read_properties("config.properties".to_string());
  }
  else { config = DataObject::new(); b = true; }
  
  if !config.has("security") { config.put_bool("security", true); b = true; }
  else { 
    let b = config.get_string("security") == "on";
    config.put_bool("security", b); 
    if !b { println!("Warning! Security is OFF!"); }
  }
  
  if !config.has("http_address") { config.put_str("http_address", "127.0.0.1"); b = true; }
  if !config.has("http_port") { config.put_str("http_port", "0"); }
  
  if config.has("sessiontimeoutmillis") { 
    let d = config.get_property("sessiontimeoutmillis");
    if !d.is_int() { 
      let session_timeout = d.string().parse::<i64>().unwrap(); 
      config.duplicate().put_i64("sessiontimeoutmillis", session_timeout);
    }
  }
  else { 
    let session_timeout = 900000; 
    config.duplicate().put_i64("sessiontimeoutmillis", session_timeout);
    b = true;
  }
  
  if !config.has("apps") {
    // FIXME - scan for app.properties files
    config.put_str("apps", "app,dev,peer,security");
    b = true;
  }
  
  if !config.has("default_app") {
    config.put_str("default_app", "app");
    b = true;
  }
  
  if !config.has("machineid") {
    config.put_str("machineid", "MY_DEVICE");
    b = true;
  }
  
  if b { save_config(config.duplicate()); }
  
  let mut system = DataStore::globals().get_object("system");
  system.put_object("config", config.duplicate());
  config
}

pub fn init_globals() -> DataObject {
  let mut globals = DataStore::globals();
  
  let mut system;
  let first_time;
  if globals.has("system") { system = globals.get_object("system"); first_time = false; }
  else {
    system = DataObject::new();
    globals.put_object("system", system.duplicate());
    first_time = true;
  }

  let config = load_config();
  
  if first_time {
    load_users();
    system.put_object("timers", DataObject::new());
    system.put_object("events", DataObject::new());
    system.put_array("fire", DataArray::new());
  }
  
  let s = config.get_string("apps");
  let s = s.trim().to_string();
  let sa = s.split(",");
  
  let mut apps = DataObject::new();
  let default_app;
  if config.has("default_app") { default_app = config.get_string("default_app"); }
  else { default_app = sa.to_owned().nth(0).unwrap().to_string(); }
  
  let libraries = DataObject::new();
  system.put_object("libraries", libraries.duplicate());
  
  for i in sa {
    let mut o = DataObject::new();
    o.put_str("id", i);
    let path_base = "runtime/".to_string()+i+"/";
    let path = path_base.to_owned()+"botd.properties";
    let p;
    let ppath = Path::new(&path);
    if ppath.exists(){ p = read_properties(path); } 
    else { 
      let _x = File::create(ppath).unwrap();
      p = DataObject::new(); 
    }
    o.put_object("runtime", p);
    let path = path_base+"app.properties";
    let p;
    let ppath = Path::new(&path);
    if ppath.exists(){ p = read_properties(path); } 
    else {
      let _x = File::create(ppath).unwrap();
      p = DataObject::new(); 
    }
    o.put_object("app", p.duplicate());
    apps.put_object(i, o);
    
    let s = p.get_string("libraries");
    let sa2 = s.split(",");
    for j in sa2 {
      if !libraries.has(j) { load_library(j); }
    }
  }
  
  system.put_str("default_app", &default_app);
  system.put_object("apps", apps);
  system.put_bool("running", true);
  
  let store = DataStore::new();

  if first_time {
      system.put_object("sessions", DataObject::new());
      
      // Init Timers and Events
      for lib in libraries.duplicate().keys() {
        let controls = store.get_data(&lib, "controls").get_object("data").get_array("list");
        for ctldata in controls.objects() {
          let ctldata = ctldata.object();
          let ctlid = ctldata.get_string("id");
          let ctlname = ctldata.get_string("name");
          let ctl = store.get_data(&lib, &ctlid).get_object("data");
          if ctl.has("timer") {
            let ctimers = ctl.get_array("timer");
            for timer in ctimers.objects() {
              let timer = timer.object();
              let tname = timer.get_string("name");
              let tid = timer.get_string("id");
              let mut tdata = store.get_data(&lib, &tid).get_object("data");
              tdata.put_str("ctlname", &ctlname);
              tdata.put_str("name", &tname);
              if !tdata.has("start") { println!("Timer {}:{}:{} is not properly configured.", lib, ctlname, tname); }
              else { add_timer(&tid, tdata); }
            }
          }
          if ctl.has("event") {
            let cevents = ctl.get_array("event");
            for event in cevents.objects() {
              let event = event.object();
              let eid = event.get_string("id");
              let edata = store.get_data(&lib, &eid).get_object("data");
              if !edata.has("bot") { println!("Event listener {}:{}:{} is not properly configured.", lib, ctlname, event.get_string("name")); }
              else {
                let app = edata.get_string("bot");
                let ename = edata.get_string("event");
                let cmddb = edata.get_string("cmddb");
                let cmdid = edata.get_string("cmd");
                add_event_listener(&eid, &app, &ename, &cmddb, &cmdid);
              }
            }
          }
        }
      }
  }
  
  let p = store.root;
  for file in fs::read_dir(&p).unwrap() {
    let path = file.unwrap().path();
    if path.is_dir() {
      let name:String = path.file_name().unwrap().to_str().unwrap().to_string();
      if !libraries.has(&name) { load_library(&name); }
    }  
  }
  
  system
}

fn to_millis(i:i64, s:String) -> i64 {
  if s == "milliseconds" { return i; }
  let i = i * 1000;
  if s == "seconds" { return i; }
  let i = i * 60;
  if s == "minutes" { return i; }
  let i = i * 60;
  if s == "hours" { return i; }
  let i = i * 24;
  if s != "days" { panic!("Unknown time unit for timer ({})", &s); }
  
  i
}

pub fn lookup_command_id(app:String, cmd: String) -> (bool, String, String) {
  let system = DataStore::globals().get_object("system");
  let mut b = false;
  let mut ctldb = "".to_string();
  let mut id = "".to_string();
  let apps = system.get_object("apps");
  if apps.has(&app) {
    let appdata = apps.get_object(&app).get_object("app");
    ctldb = appdata.get_string("ctldb");
    let ctlid = appdata.get_string("ctlid");
    let store = DataStore::new();
    let ctllist = store.get_data(&ctldb, &ctlid).get_object("data");
    if ctllist.has("cmd") {
      let ctllist = ctllist.get_array("cmd");
      for ctl in ctllist.objects() {
        let ctl = ctl.object();
        let name = ctl.get_string("name");
        if name == cmd {
          b = true;
          id = ctl.get_string("id");
          break;
        }
      }
      }
  }
  (b, ctldb, id)
}

