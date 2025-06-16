// In flowlang/src/rustcmd.rs

use ndata::dataobject::*;

use crate::code::*;
use crate::DataStore;


pub type Transform = fn(DataObject) -> DataObject;

#[derive(Debug)]
pub struct RustCmd {
    func: Transform,
}

impl RustCmd {
    pub fn detail(_id: String, t: Transform, io: String) -> DataObject {
        // Create a new object to hold the command's details.
        let mut cmd_details = DataObject::new();

        // Store the function pointer by casting it to an integer.
        cmd_details.put_int("transform_ptr", t as i64);
        cmd_details.put_string("io", &io);

        cmd_details
    }

    pub fn new(id: &str) -> RustCmd {
      let cmd_map = DataStore::globals().get_object("RUST_COMMANDS");

      if !cmd_map.has(id) {
          panic!("No such command {}", id);
      }

      let cmd_details = cmd_map.get_object(id);
      let ptr_val = cmd_details.get_int("transform_ptr");

      // Unsafely cast the integer back into a function pointer.
      unsafe {
        let func_ptr: Transform = std::mem::transmute(ptr_val);

        RustCmd {
            func: func_ptr,
        }
      }
    }

    pub fn exists(id: &str) -> bool {
      let cmd_map = DataStore::globals().get_object("RUST_COMMANDS");
      cmd_map.has(id)
    }

    pub fn execute(&self, args: DataObject) -> Result<DataObject, CodeException> {
        Ok((self.func)(args))
    }
}
