use std::collections::HashMap;
use ndata::data::*;
use ndata::dataobject::*;
#[cfg(feature="serde_support")]
use serde::*;

#[derive(Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Case {
  pub input: HashMap<String, Node>,
  pub output: HashMap<String, Node>,
  pub cmds: Vec<Operation>,
  pub cons: Vec<Connection>,
  pub nextcase: Option<Box<Case>>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Node {
  #[cfg_attr(feature = "serde_support", serde(default))]
  pub mode: String,
  #[cfg_attr(feature = "serde_support", serde(default))]
  #[cfg_attr(feature = "serde_support", serde(rename = "type"))]
  pub cmd_type: String,
  #[cfg_attr(feature = "serde_support", serde(default))]
  pub x:f32,
  #[cfg_attr(feature = "serde_support", serde(skip))]
  pub val:Data,
  #[cfg_attr(feature = "serde_support", serde(default))]
  pub done: bool,
  pub list: Option<String>,
  #[cfg_attr(feature = "serde_support", serde(rename = "loop"))]
  pub looop: Option<String>,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Operation {
  #[cfg_attr(feature = "serde_support", serde(rename = "in"))]
  pub input: HashMap<String, Node>,
  #[cfg_attr(feature = "serde_support", serde(rename = "out"))]
  pub output: HashMap<String, Node>,
  pub pos: Pos,
  pub name: String,
  pub width: f64,
  #[cfg_attr(feature = "serde_support", serde(rename = "type"))]
  pub cmd_type: String,
  pub ctype: Option<String>,
  pub cmd: Option<String>,
  pub localdata: Option<Box<Case>>,
  #[cfg_attr(feature = "serde_support", serde(skip))]
  pub result: Option<DataObject>,
  pub condition: Option<Condition>,
  #[cfg_attr(feature = "serde_support", serde(default))]
  pub done: bool,
  #[cfg_attr(feature = "serde_support", serde(default))]
  pub finish: bool,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Connection {
  pub src: Dest,
  pub dest: Dest,

  #[cfg_attr(feature = "serde_support", serde(default))]
  pub done: bool,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Pos {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Dest {
  pub index: i64,
  pub name: String,
}

#[derive(Debug)]
#[cfg_attr(feature = "serde_support", derive(Serialize, Deserialize))]
pub struct Condition {
  pub value: bool,
  pub rule: String,
}

impl Connection {
  pub fn from_data(data:DataObject) -> Connection {
    let s = data.get_array("src");
    let src =  Dest{
      index: s.get_i64(0),
      name: s.get_string(1),
    };
    let s = data.get_array("dest");
    let dest =  Dest{
      index: s.get_i64(0),
      name: s.get_string(1),
    };
    let done = false;
    Connection {
      src: src,
      dest: dest,
      done: done,
    }
  }

  pub fn duplicate(&self) -> Connection {
    let src =  Dest{
      index: self.src.index,
      name: self.src.name.to_owned(),
    };
    let dest =  Dest{
      index: self.dest.index,
      name: self.dest.name.to_owned(),
    };
    let done = false;
    Connection {
      src: src,
      dest: dest,
      done: done,
    }
  }
}


impl Condition {
  pub fn from_data(data:DataObject) -> Condition {
    Condition {
      value: data.get_bool("value"),
      rule: data.get_string("rule"),
    }
  }

  pub fn duplicate(&self) -> Condition {
    Condition {
      value: self.value,
      rule: self.rule.to_owned(),
    }
  }
}

impl Operation {
  pub fn from_data(data:DataObject) -> Operation {
    let mut input = HashMap::<String, Node>::new();
    let mut output = HashMap::<String, Node>::new();
    let p = data.get_object("pos");
    let pos = Pos{
      x: p.get_f64("x"),
      y: p.get_f64("y"),
      z: p.get_f64("z"),
    };
    let name = data.get_string("name");
    let width = data.get_f64("width");
    let cmd_type = data.get_string("type");
    let mut ctype: Option<String> = None;
    let mut cmd: Option<String> = None;
    let mut localdata: Option<Box<Case>> = None;
    let result = None;
    let mut condition: Option<Condition> = None;
    let done = false;
    let finish = false;
    
    for (k, node) in data.get_object("in").objects() {
      input.insert(k.to_string(), Node::from_data(node.object()));
    }
    
    for (k, node) in data.get_object("out").objects() {
      output.insert(k.to_string(), Node::from_data(node.object()));
    }
    
    if data.has("ctype") { ctype = Some(data.get_string("ctype")); }
    if data.has("cmd") { cmd = Some(data.get_string("cmd")); }
    if data.has("localdata") { localdata = Some(Box::new(Case::from_data(data.get_object("localdata")))); }
    if data.has("condition") { condition = Some(Condition::from_data(data.get_object("condition"))); }
    
    Operation {
      input: input,
      output: output,
      pos: pos,
      name: name,
      width: width,
      cmd_type: cmd_type,
      ctype: ctype,
      cmd: cmd,
      localdata: localdata,
      result: result,
      condition: condition,
      done: done,
      finish: finish,
    }  
  }

  pub fn duplicate(&self) -> Operation {
    let mut input = HashMap::<String, Node>::new();
    let mut output = HashMap::<String, Node>::new();
    let pos = Pos{
      x: self.pos.x,
      y: self.pos.y,
      z: self.pos.z,
    };
    let name = self.name.to_owned();
    let width = self.width;
    let cmd_type = self.cmd_type.to_owned();
    let mut ctype: Option<String> = None;
    let mut cmd: Option<String> = None;
    let mut localdata: Option<Box<Case>> = None;
    let result = None;
    let mut condition: Option<Condition> = None;
    let done = false;
    let finish = false;
    
    for (k, node) in &self.input {
      input.insert(k.to_string(), node.duplicate());
    }
    
    for (k, node) in &self.output {
      output.insert(k.to_string(), node.duplicate());
    }
    
    if !self.ctype.is_none() { ctype = Some(self.ctype.as_ref().unwrap().to_owned()); }
    if !self.cmd.is_none() { cmd = Some(self.cmd.as_ref().unwrap().to_owned()); }
    if !self.localdata.is_none() { localdata = Some(Box::new(self.localdata.as_ref().unwrap().duplicate())); }
    if !self.condition.is_none() { condition = Some(self.condition.as_ref().unwrap().duplicate()); }
    
    Operation {
      input: input,
      output: output,
      pos: pos,
      name: name,
      width: width,
      cmd_type: cmd_type,
      ctype: ctype,
      cmd: cmd,
      localdata: localdata,
      result: result,
      condition: condition,
      done: done,
      finish: finish,
    }  
  }
}

impl Node {
  pub fn from_data(data:DataObject) -> Node {
    let mode = data.get_string("mode");
    let cmd_type = data.get_string("type");
    let x = data.get_f64("x") as f32;
    let val = Data::DNull;
    let done = false;
    let mut list: Option<String> = None;
    let mut looop: Option<String> = None;
    
    if data.has("list") {
      list = Some(data.get_string("list"));
    }
    
    if data.has("loop") {
      looop = Some(data.get_string("loop"));
    }
    
    Node {
      mode: mode,
      cmd_type: cmd_type,
      x: x,
      val: val,
      done: done,
      list: list,
      looop: looop,
    }
  }

  pub fn duplicate(&self) -> Node {
    let mode = self.mode.to_owned();
    let cmd_type = self.cmd_type.to_owned();
    let x = self.x;
    let val = Data::DNull;
    let done = false;
    let mut list: Option<String> = None;
    let mut looop: Option<String> = None;
    
    if !self.list.is_none() {
      list = Some(self.list.as_ref().unwrap().to_owned());
    }
    
    if !self.looop.is_none() {
      looop = Some(self.looop.as_ref().unwrap().to_owned());
    }
    
    Node {
      mode: mode,
      cmd_type: cmd_type,
      x: x,
      val: val,
      done: done,
      list: list,
      looop: looop,
    }
  }
}

impl Case {
  pub fn new(data: &str) -> Case {
    #[cfg(feature="serde_support")]
    return serde_json::from_str(data).unwrap(); 
    
    #[allow(unreachable_code)]
    {
      let data = DataObject::from_string(data);
      return Case::from_data(data);
    }
  }
  
  pub fn from_data(data:DataObject) -> Case {
    let mut input = HashMap::<String, Node>::new();
    let mut output = HashMap::<String, Node>::new();
    let mut cmds = Vec::<Operation>::new();
    let mut cons = Vec::<Connection>::new();
    let mut nextcase = None;
    
    for (k, node) in data.get_object("input").objects() {
      input.insert(k.to_string(), Node::from_data(node.object()));
    }
    
    for (k, node) in data.get_object("output").objects() {
      output.insert(k.to_string(), Node::from_data(node.object()));
    }
    
    for op in data.get_array("cmds").objects() {
      cmds.push(Operation::from_data(op.object()));
    }
    
    for con in data.get_array("cons").objects() {
      cons.push(Connection::from_data(con.object()));
    }
    
    if data.has("nextcase") {
      nextcase = Some(Box::new(Case::from_data(data.get_object("nextcase"))));
    }
    
    Case {
      input: input,
      output: output,
      cmds: cmds,
      cons: cons,
      nextcase: nextcase,
    }
  }
  
  pub fn duplicate(&self) -> Case {
    let mut input = HashMap::<String, Node>::new();
    let mut output = HashMap::<String, Node>::new();
    let mut cmds = Vec::<Operation>::new();
    let mut cons = Vec::<Connection>::new();
    let mut nextcase = None;
    
    for (k, node) in &self.input {
      input.insert(k.to_string(), node.duplicate());
    }
    
    for (k, node) in &self.output {
      output.insert(k.to_string(), node.duplicate());
    }
    
    for op in &self.cmds {
      cmds.push(op.duplicate());
    }
    
    for con in &self.cons {
      cons.push(con.duplicate());
    }
    
    if !self.nextcase.is_none() {
      nextcase = Some(Box::new(self.nextcase.as_ref().unwrap().duplicate()));
    }
    
    Case {
      input: input,
      output: output,
      cmds: cmds,
      cons: cons,
      nextcase: nextcase,
    }
  }
}

