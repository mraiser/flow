use ndata::dataobject::*;
use ndata::dataarray::*;
use ndata::databytes::*;
use crate::case::*;

/// Why a flow failed. `Code::execute` and `Command::execute` return it as
/// `CodeException::Fail(FlowError)`.
#[derive(PartialEq, Debug, Clone)]
pub enum CodeException {
    Fail(FlowError),
    Terminate,
    NextCase,
}

impl std::fmt::Display for CodeException {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CodeException::Fail(e) => write!(f, "{}", e),
      CodeException::Terminate => write!(f, "the flow terminated"),
      CodeException::NextCase => write!(f, "the flow moved to its next case"),
    }
  }
}

impl CodeException {
  /// The error response a caller hands back for this failure:
  /// `{"status": "err", "msg": <one line>, "flow_error": <FlowError::to_data>}`.
  pub fn to_envelope(&self) -> DataObject {
    match self {
      CodeException::Fail(e) => e.to_envelope(),
      other => {
        let mut o = DataObject::new();
        o.put_string("status", "err");
        o.put_string("msg", &other.to_string());
        o
      }
    }
  }
}

/// One level of the frame stack, outermost first: an op in a case of a flow. A failure inside a
/// local op or a flow command op has one site per frame it passed through, ending at the op
/// (or case) where it happened.
#[derive(PartialEq, Debug, Clone)]
pub struct FlowSite {
  /// the case's index in its frame's case chain (0 is the first case)
  pub case: usize,
  /// the op's index in the case's `cmds`; None when the failure belongs to the case as a whole
  /// (a stuck case, a `next` with no next case, a wire)
  pub op: Option<usize>,
  /// the iteration of a list or loop op (0 is the first)
  pub iter: Option<i64>,
  /// the op's type (primitive, command, local, constant, persistent, match, ...)
  pub op_type: String,
  /// the op's name: the primitive, the constant's literal, the local's or command's label
  pub op_name: String,
}

/// A flow's failure: what went wrong, and where.
///
/// `code` is one of:
/// - "fail": an op's `fail` rule fired. `cause` is the failure of the local or flow command
///   that made the op's condition fire, when there was one.
/// - "stall": the case can never finish (I-1 an input awaited by name that no wire reaches,
///   or I-2 a cycle).
/// - "I-4": a `next` rule fired in the last case of the chain.
/// - "absent-output": a wire or a command op read an output the op's result lacks.
/// - "panic": a primitive, command or literal failed, or the graph is malformed.
/// - "depth": frames nested more than 64 deep.
/// - "budget": the run took more than its step budget.
#[derive(PartialEq, Debug, Clone)]
pub struct FlowError {
  pub code: String,
  pub msg: String,
  /// where it happened, outermost frame first; empty when the run as a whole failed (budget)
  pub at: Vec<FlowSite>,
  pub cause: Option<Box<FlowError>>,
}

impl FlowSite {
  fn describe(&self) -> String {
    let mut s = String::new();
    if self.case > 0 { s.push_str(&format!("case {} ", self.case)); }
    match self.op {
      Some(i) => s.push_str(&format!("op {} {} {:?}", i, self.op_type, self.op_name)),
      None => s.push_str(if self.case > 0 { "itself" } else { "case 0" }),
    }
    if let Some(it) = self.iter { s.push_str(&format!(" (iteration {})", it)); }
    s
  }

  pub fn to_data(&self) -> DataObject {
    let mut o = DataObject::new();
    o.put_int("case", self.case as i64);
    match self.op { Some(i) => o.put_int("op", i as i64), None => o.put_null("op") }
    match self.iter { Some(i) => o.put_int("iter", i), None => o.put_null("iter") }
    o.put_string("type", &self.op_type);
    o.put_string("name", &self.op_name);
    o
  }
}

impl FlowError {
  /// Put this failure (and its causes) inside `site`, the op of an enclosing frame.
  pub fn within(&mut self, site: FlowSite) {
    if let Some(c) = &mut self.cause { c.within(site.clone()); }
    self.at.insert(0, site);
  }

  /// `msg` for people: without the `op N: ` that repeats the location, and without the
  /// stepper's notes on how older flowlang versions crashed ("(flowlang panics ...)").
  /// `msg` itself is the stepper's text, unchanged.
  pub fn detail(&self) -> String {
    let mut m = self.msg.as_str();
    if let Some(Some(i)) = self.at.last().map(|s| s.op) {
      let lead = format!("op {}: ", i);
      if let Some(rest) = m.strip_prefix(lead.as_str()) { m = rest; }
    }
    let mut out = m.to_string();
    while let Some(p) = out.find(" (flowlang panics") {
      match out[p..].find(')') { Some(q) => out.replace_range(p..p + q + 1, ""), None => break }
    }
    out
  }

  /// The path to the failure, e.g. `op 1 local "check" (iteration 2) > op 0 primitive "+"`.
  pub fn location(&self) -> String {
    self.at.iter().map(|s| s.describe()).collect::<Vec<_>>().join(" > ")
  }

  pub fn to_data(&self) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("code", &self.code);
    o.put_string("msg", &self.detail());
    let mut at = DataArray::new();
    for s in &self.at { at.push_object(s.to_data()); }
    o.put_array("at", at);
    match &self.cause { Some(c) => o.put_object("cause", c.to_data()), None => o.put_null("cause") }
    o
  }

  /// `{"status": "err", "msg": <to_string()>, "flow_error": <to_data()>}`
  pub fn to_envelope(&self) -> DataObject {
    let mut o = DataObject::new();
    o.put_string("status", "err");
    o.put_string("msg", &self.to_string());
    o.put_object("flow_error", self.to_data());
    o
  }
}

/// One line: `flow failed (<code>) at <location>: <msg>[; because <cause>]`.
impl std::fmt::Display for FlowError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "flow failed ({})", self.code)?;
    if !self.at.is_empty() { write!(f, " at {}", self.location())?; }
    write!(f, ": {}", self.detail())?;
    if let Some(c) = &self.cause { write!(f, "; because {}", c)?; }
    Ok(())
  }
}

impl std::error::Error for FlowError {}

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

  /// Execute this case chain: the stepper's continue-to-end on the default pick (spec:
  /// docs/flow-language.md 1.5). `crate::stepper` defines the semantics one step at a time, for
  /// the debugger; this runs the same steps through `crate::fastrun`, which keeps its bookkeeping
  /// native instead of building the stepper's ndata frames and events (tests/fastrun_parity.rs
  /// holds the two to the same results, errors and step counts). A `next` with no next case (I-4)
  /// and a stuck case (I-1/I-2) return `Err(CodeException::Fail)`; a root `terminate` ends the
  /// flow with the result delivered so far.
  pub fn execute(&mut self, args: DataObject) -> Result<DataObject, CodeException> {
    Code::execute_case(&self.data, args, &mut self.finishflag)
  }

  /// `execute` on a borrowed case, so a caller holding a `Case` need not duplicate it.
  pub fn execute_case(case: &Case, args: DataObject, finishflag: &mut bool) -> Result<DataObject, CodeException> {
    let (r, finish) = crate::fastrun::execute(case, args);
    *finishflag = finish;
    DataObject::gc();
    DataArray::gc();
    DataBytes::gc();
    r
  }

}
