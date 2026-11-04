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
