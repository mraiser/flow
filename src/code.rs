use ndata::dataobject::*;
use ndata::dataarray::*;
use ndata::databytes::*;
use crate::case::*;

#[derive(PartialEq, Debug)]
pub enum CodeException {
    Fail,
    Terminate,
    NextCase,
}

#[derive(Debug)]
pub struct Code {
  pub data: Case,
  pub finishflag: bool,
}

impl Code {
  pub fn new(data: Case) -> Code {
    Code {
      data: data,
      finishflag: false,
    }
  }

  /// Execute this case chain: a continue-to-end over the stepper (Flow step debugger P6).
  /// The stepper (`crate::stepper`) is the single source of truth for flow execution; this builds
  /// the root frame and runs it to completion on the default pick. Semantics are the language's
  /// (docs/flow-language.md 1.5), with two hardenings the stepper already proved: a `next` with no
  /// next case (I-4) and a stuck case (I-1/I-2) now return `Err(CodeException::Fail)` instead of
  /// panicking / hanging the thread. A nested `Terminate` propagates; at the root it ends the flow
  /// with the result delivered so far.
  pub fn execute(&mut self, args: DataObject) -> Result<DataObject, CodeException> {
    let body = self.data.to_data();
    let mut frames = DataArray::new();
    frames.push_object(crate::stepper::new_frame(args, "root", body, DataArray::new()));
    let r = crate::stepper::run(&mut frames, 0);
    self.finishflag = frames.get_object(0).get_boolean("finish");
    DataObject::gc();
    DataArray::gc();
    DataBytes::gc();
    if r.outcome == "fail" {
      let e = &r.event;
      let code = if e.has("code") { e.get_string("code") } else { "fail".to_string() };
      let msg = if e.has("msg") { e.get_string("msg") } else { String::new() };
      match code.as_str() {
        "fail" | "I-4" => return Err(CodeException::Fail),
        "terminate" => return Err(CodeException::Terminate),
        "next" => return Err(CodeException::NextCase),
        _ => { println!("FLOW ERROR [{}]: {}", code, msg); return Err(CodeException::Fail); }
      }
    }
    Ok(r.out)
  }
    
  // The step-by-step propagation loop, op evaluation and conditionals that used to live here
  // moved into the stepper (crate::stepper) with Flow step debugger P6; `execute` above is the
  // continue-to-end over it. The language semantics are unchanged (docs/flow-language.md 1.5).
}
