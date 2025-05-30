use ndata::dataobject::*;
use std::thread;
use std::panic;
use std::fs;
use std::path::Path;
//use std::fs::File;
use ndata::dataarray::*;
//use ndata::sharedmutex::SharedMutex;
use std::time::Duration;
//use state::Storage;
use std::sync::RwLock;
//use std::sync::Once;

use crate::command::*;
use crate::datastore::*;

use crate::flowlang::system::time::time;
use crate::flowlang::file::read_properties::read_properties;
use crate::flowlang::file::write_properties::write_properties;

// FIXME - The code in this file makes the assumption in several places that the process was launched from the root directory. That assumption should only be made once, in the event that no root directory is specified, by whatever initializes the flowlang DataStore.

pub type EventHook = fn(&str,&str,DataObject);

//static START: Once = Once::new();
//pub static EVENTHOOKS:Storage<RwLock<Vec<EventHook>>> = Storage::new();
static mut EVENTHOOKS:RwLock<Option<Vec<EventHook>>> = RwLock::new(Some(Vec::new()));

pub fn run() {
  let system = init_globals();
  
  // Start Timers
  thread::spawn(timer_loop);
  
  // Start Events
  thread::spawn(event_loop);
  
  let dur = Duration::from_millis(500);
  while system.get_boolean("running") {
    thread::sleep(dur);
  }
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
  let start = tdata.get_int("start");
  let start = time()+to_millis(start, tdata.get_string("startunit"));
  let interval = tdata.get_int("interval");
  let interval = to_millis(interval, tdata.get_string("intervalunit"));
  tdata.put_int("startmillis", start);
  tdata.put_int("intervalmillis", interval);
  timers.put_object(&tid, tdata);
}

#[allow(static_mut_refs)]
pub fn add_event_hook(hook:EventHook) {
  unsafe { 
    let map = &mut EVENTHOOKS.write().unwrap();
    let map = map.as_mut().unwrap();
    map.push(hook);    
  }
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
    events.put_object(app, bot.clone());
  }
  if bot.has(event) {
    list = bot.get_object(event);
  }
  else {
    list = DataObject::new();
    bot.put_object(event, list.clone());
  }
  let mut cmd = DataObject::new();
  cmd.put_string("lib", cmdlib);
  cmd.put_string("cmd", cmdid);
  list.put_object(id, cmd);
}

pub fn fire_event(app:&str, event:&str, data:DataObject) {
  let mut o = DataObject::new();
  o.put_string("app", &app);
  o.put_string("event", &event);
  o.put_object("data", data);
  DataStore::globals().get_object("system").get_array("fire").push_object(o);
}

#[allow(static_mut_refs)]
pub fn event_loop() {
  let system = DataStore::globals().get_object("system");
  let mut events = system.get_object("events");
  let fire = system.get_array("fire");
  let dur = Duration::from_millis(100);
  while system.get_boolean("running") {
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
          let data = data.clone();
          thread::spawn(move || {
            let command = Command::new(&lib, &id);
            let  _ = command.execute(data);
          });
        }
      }
      unsafe {
        for hook in EVENTHOOKS.read().unwrap().as_ref().unwrap().iter() {
          hook(&app, &event, o.clone());
        }
      }
    }
        
    if b { thread::sleep(dur); }
  }
}

fn timer_loop() {
  let system = DataStore::globals().get_object("system");
  let dur = Duration::from_millis(1000);
  while system.get_boolean("running") {
    let now = time();
    let mut timers = system.get_object("timers");
    for (id, timer) in timers.objects() {
      let mut timer = timer.object();
      let when = timer.get_int("startmillis");
      if when <= now {
        let cmdid = timer.get_string("cmd");
        let db = timer.get_string("cmddb");
        if Command::exists(&db, &cmdid) {
          timers.remove_property(&id);
          let params = timer.get_object("params");
          let repeat = timer.get_boolean("repeat");
          let mut ts = timers.clone();
          thread::spawn(move || {
            let cmd = Command::new(&db, &cmdid);
            let _x = cmd.execute(params).unwrap();
              
            if repeat {
              let next = now + timer.get_int("intervalmillis");
              timer.put_int("startmillis", next);
              ts.put_object(&id, timer);
            }
          });
        }
        else { 
          println!("Event command {}:{} not found, removing", &db, &cmdid); 
          timers.remove_property(&id);
        }
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
    for r in o2.get_array("readers").objects() { readers.push_string(&(r.string())); }
    }
    if o2.has("writers") { 
    for w in o2.get_array("writers").objects() { writers.push_string(&(w.string())); }
    }
    o2.put_array("readers", readers);
    o2.put_array("writers", writers);
    o2.put_string("id", j);

    let mut libraries = system.get_object("libraries");
    libraries.put_object(j, o2);
  }
}

pub fn save_config(config: DataObject) {
  let mut config = config.deep_copy();
  if !config.has("security") { config.put_string("security", "on"); }
  else { 
    let b = config.get_boolean("security");
    if b { config.put_string("security", "on"); } 
    else { config.put_string("security", "off"); } 
  }
  write_properties("config.properties".to_string(), config);
}

pub fn load_config() -> DataObject {
  //println!("Loading appserver configuration");
  let mut b = false;
  let mut config;
  if Path::new("config.properties").exists() {
    config = read_properties("config.properties".to_string());
  }
  else { config = DataObject::new(); b = true; }
  
  if !config.has("security") { config.put_boolean("security", true); b = true; }
  else { 
    let b = config.get_string("security") == "on";
    config.put_boolean("security", b); 
    if !b { println!("Warning! Security is OFF!"); }
  }
  
  if !config.has("http_address") { config.put_string("http_address", "0.0.0.0"); b = true; }
  if !config.has("http_port") { config.put_string("http_port", "0"); }
  
  if config.has("sessiontimeoutmillis") { 
    let d = config.get_property("sessiontimeoutmillis");
    if !d.is_int() { 
      let session_timeout = d.string().parse::<i64>().unwrap(); 
      config.clone().put_int("sessiontimeoutmillis", session_timeout);
    }
  }
  else { 
    let session_timeout = 900000; 
    config.clone().put_int("sessiontimeoutmillis", session_timeout);
    b = true;
  }
  
  if !config.has("apps") {
    // FIXME - scan for app.properties files
    config.put_string("apps", "app,dev,peer,security");
    b = true;
  }
  
  if !config.has("default_app") {
    config.put_string("default_app", "app");
    b = true;
  }
  
  if !config.has("machineid") {
    config.put_string("machineid", "MY_DEVICE");
    b = true;
  }
  
  if b { save_config(config.clone()); }
  
  let mut system = DataStore::globals().get_object("system");
  system.put_object("config", config.clone());
  config
}

pub fn init_globals() -> DataObject {
  let mut globals = DataStore::globals();
  
  let mut system;
  let first_time;
  if globals.has("system") { system = globals.get_object("system"); first_time = false; }
  else {
    system = DataObject::new();
    globals.put_object("system", system.clone());
    first_time = true;
  }

  let config = load_config();
  
  if first_time {
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
  system.put_object("libraries", libraries.clone());
  
  for i in sa {
    let mut o = DataObject::new();
    o.put_string("id", i);
    let path_base = "runtime/".to_string()+i+"/";
    let path = path_base.to_owned()+"botd.properties";
    let p;
    let ppath = Path::new(&path);
    if ppath.exists(){ p = read_properties(path); } 
    else { 
      //let _x = File::create(ppath).unwrap();
      p = DataObject::new(); 
    }
    o.put_object("runtime", p);
    let path = path_base+"app.properties";
    let p;
    let ppath = Path::new(&path);
    if ppath.exists(){ p = read_properties(path); } 
    else {
      //let _x = File::create(ppath).unwrap();
      p = DataObject::new(); 
    }
    o.put_object("app", p.clone());
    apps.put_object(i, o);
    
    if p.has("libraries"){
      let s = p.get_string("libraries");
      let sa2 = s.split(",");
      for j in sa2 {
        if !libraries.has(j) { load_library(j); }
      }
    }
  }
  
  system.put_string("default_app", &default_app);
  system.put_object("apps", apps);
  system.put_boolean("running", true);
  
  let store = DataStore::new();

  if first_time {
      system.put_object("sessions", DataObject::new());
      
      // Init Timers and Events
      for lib in libraries.clone().keys() {
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
              tdata.put_string("ctlname", &ctlname);
              tdata.put_string("name", &tname);
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

