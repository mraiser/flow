appserver::init_globals();

// ── serverInfo ────────────────────────────────────────────────────────────
let mut server_info = DataObject::new();
server_info.put_string("name", "newbound-mcp"); // whatever you like
server_info.put_string("version", "0.1.0");

// ── capabilities → tools.listChanged = false ─────────────────────────────
let mut tools_caps = DataObject::new();
tools_caps.put_bool("listChanged", false);

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