use serde::*;
use serde_json::*;
use std::collections::HashMap;

use ndata::data::*;
use ndata::dataobject::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Case {
  pub input: HashMap<String, Node>,
  pub output: HashMap<String, Node>,
  pub cmds: Vec<Operation>,
  pub cons: Vec<Connection>,
  pub nextcase: Option<Box<Case>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
  #[serde(default)]
  pub mode: String,
  #[serde(default)]
  #[serde(rename = "type")]
  pub cmd_type: String,
  #[serde(default)]
  pub x:f32,
  #[serde(skip)]
  pub val:Data,
  #[serde(default)]
  pub done: bool,
  pub list: Option<String>,
  #[serde(rename = "loop")]
  pub looop: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Operation {
  #[serde(rename = "in")]
  pub input: HashMap<String, Node>,
  #[serde(rename = "out")]
  pub output: HashMap<String, Node>,
  pub pos: Pos,
  pub name: String,
  pub width: f64,
  #[serde(rename = "type")]
  pub cmd_type: String,
  pub ctype: Option<String>,
  pub cmd: Option<String>,
  pub localdata: Option<Box<Case>>,
  #[serde(skip)]
  pub result: Option<DataObject>,
  pub condition: Option<Condition>,
  #[serde(default)]
  pub done: bool,
  #[serde(default)]
  pub finish: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Connection {
  pub src: Dest,
  pub dest: Dest,

  #[serde(default)]
  pub done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Pos {
  pub x: f64,
  pub y: f64,
  pub z: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dest {
  pub index: i64,
  pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Condition {
  pub value: bool,
  pub rule: String,
}

impl Connection {
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
  pub fn duplicate(&self) -> Condition {
    Condition {
      value: self.value,
      rule: self.rule.to_owned(),
    }
  }
}

impl Operation {
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
  pub fn new(data: &str) -> Result<Case> {
    let c: Case = serde_json::from_str(data)?;
    Ok(c)
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

