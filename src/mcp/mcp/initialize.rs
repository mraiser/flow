use ndata::dataobject::DataObject;
//use crate::appserver;

pub fn execute(_: DataObject) -> DataObject {
    use std::panic;
    let ax = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        initialize()
    }));
    match ax {
        Ok(ax) => {
            let mut result_obj = DataObject::new();
    result_obj.put_object("a", ax);
            result_obj
        }
        Err(err) => {
            let mut err_obj = DataObject::new();
            err_obj.put_string("status", "err");

            let msg = if let Some(s) = err.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = err.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic occurred".to_string()
            };

            err_obj.put_string("msg", &msg);
            // Wrapped in the same `a` envelope a successful return uses.
            // Unwrapped, callers that unpack the envelope (newbound's
            // format_result, for one) report an opaque 500 — "Not an object:
            // DString(\"err\")" — instead of this message.
            let mut result_obj = DataObject::new();
            result_obj.put_object("a", err_obj);
            result_obj
        }
    }
}

pub fn initialize() -> DataObject {
//appserver::init_globals();

// ── serverInfo ────────────────────────────────────────────────────────────
let mut server_info = DataObject::new();
server_info.put_string("name", "newbound-mcp"); // whatever you like
server_info.put_string("version", "0.1.0");

// ── capabilities → tools.listChanged = false ─────────────────────────────
let mut tools_caps = DataObject::new();
tools_caps.put_boolean("listChanged", false);

let mut capabilities = DataObject::new();
capabilities.put_object("tools", tools_caps);

// ── root result object ────────────────────────────────────────────────────
let mut result = DataObject::new();
result.put_string("protocolVersion", "2024-11-05"); //"2025-03-26");
result.put_object("capabilities", capabilities);
result.put_object("serverInfo", server_info);

// (Optional) onboarding instructions, promptsVersion, resourcesUri, … 
// can be added here later if you want.

result
}
