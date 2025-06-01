use ndata::dataobject::DataObject;
use crate::command::Command;
use ndata::dataarray::DataArray;
use std::path::Path;
use std::fs;
use crate::base64::*; // For Base64::encode
use ndata::Data;

pub fn execute(o: DataObject) -> DataObject {
  let arg_0: DataObject = o.get_object("data");
  let ax = invoke(arg_0);
  let mut result_obj = DataObject::new();
  result_obj.put_object("a", ax);
  result_obj
}

pub fn invoke(data: DataObject) -> DataObject {
    // ── normalise param names ────────────────────────────────────────────────
    let tool      = data.get_string("name");          // spec key
    let arguments = data.get_object("arguments");     // spec key

    // guard: badly-formed name (lib.control.command)
    let parts: Vec<&str> = tool.split('-').collect();
    if parts.len() != 3 {
        return make_error(format!("Invalid tool name '{}'", tool));
    }
    let (lib, control, command) = (parts[0], parts[1], parts[2]);

    // ── dispatch to Flowlang command impl ────────────────────────────────────
    //appserver::init_globals();
    let cmd = Command::lookup(lib, control, command);
    let result = cmd.execute(arguments);

    match result {
        Ok(v)  => wrap_value(v, cmd.return_type),
        Err(e) => make_error(format!("{:?}", e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// helpers
// ─────────────────────────────────────────────────────────────────────────────

fn wrap_value(v_input: DataObject, typ: String) -> DataObject {
    if typ == "File" {
        let file_uri_data = if v_input.has("data") {
            v_input.get_property("data")
        } else {
            eprintln!("'File' type missing 'data' key in input: {}", v_input.to_string());
            let mut error_payload = DataObject::new();
            error_payload.put_string("type", "text"); // Consistent error type
            error_payload.put_string("text", "Error: 'File' type input missing 'data' key.");

            let mut content_arr = DataArray::new();
            content_arr.push_object(error_payload);
            let mut out = DataObject::new();
            out.put_array("content", content_arr);
            return out;
        };

        let file_uri = ndata::Data::as_string(file_uri_data);

        if file_uri.starts_with("file://") {
            let file_path_str = match file_uri.strip_prefix("file://") {
                Some(p) => p,
                None => &file_uri, // Should not happen if starts_with is true
            };

            let path = Path::new(file_path_str);
            let filename = path.file_name().map_or_else(|| "unknown_file".to_string(), |s| s.to_string_lossy().into_owned());

            let extension = path.extension().map_or_else(|| "".to_string(), |s| s.to_string_lossy().into_owned());
            // Ensure mime_str gets a default like "application/octet-stream" if detection fails
            let mime_str = if !extension.is_empty() {
                let detected_mime = crate::flowlang::file::mime_type::mime_type(format!(".{}", extension));
                if detected_mime.is_empty() || !detected_mime.contains('/') {
                    "application/octet-stream".to_string()
                } else {
                    detected_mime
                }
            } else {
                "application/octet-stream".to_string()
            };

            match fs::read(file_path_str) {
                Ok(bytes_vec) => { // bytes_vec is Vec<u8>
                    // Base64::encode takes Vec<u8> by value.
                    let base64_encoded_string: String = Base64::encode(bytes_vec)
                                                          .into_iter()
                                                          .collect();

                    // Construct the Data URL. All data is base64 encoded for simplicity.
                    // Format: data:[<mediatype>][;base64],<data>
                    //let data_url_string = format!("data:{};base64,{}", mime_str, base64_encoded_string);

                    // Determine the primary MIME type (e.g., "image", "text", "application")
                    let mut parts = mime_str.splitn(2, '/');
                    let primary_mime_type = parts.next().unwrap_or("application").to_string();

                    let mut item_payload = DataObject::new();
                    item_payload.put_string("type", &primary_mime_type);
                    item_payload.put_string("data", &base64_encoded_string);
                    item_payload.put_string("filename", &filename); // Include original filename
                    item_payload.put_string("mimeType", &mime_str); // Include original filename

                    let mut content_arr = DataArray::new();
                    content_arr.push_object(item_payload);
                    let mut out = DataObject::new();
                    out.put_array("content", content_arr);
                    out
                }
                Err(e) => {
                    eprintln!("Error reading file {}: {}", file_path_str, e);
                    let mut error_payload = DataObject::new();
                    error_payload.put_string("type", "text"); // Consistent error type
                    error_payload.put_string("text", &format!("Error reading file '{}': {}", filename, e));

                    let mut content_arr = DataArray::new();
                    content_arr.push_object(error_payload);
                    let mut out = DataObject::new();
                    out.put_array("content", content_arr);
                    out
                }
            }
        } else {
            eprintln!("Invalid file URI for 'File' type: {}. Expected 'file://...' format.", file_uri);
            let mut error_payload = DataObject::new();
            error_payload.put_string("type", "text"); // Consistent error type
            error_payload.put_string("text", &format!("Invalid file URI: {}. Must start with 'file://'.", file_uri));

            let mut content_arr = DataArray::new();
            content_arr.push_object(error_payload);
            let mut out = DataObject::new();
            out.put_array("content", content_arr);
            out
        }
    } else {
        // Original logic for non-"File" types
        let data_to_wrap = if v_input.has("data") { v_input.get_property("data") }
        else if v_input.has("a") { v_input.get_property("a") }
        else if v_input.has("msg") { v_input.get_property("msg") }
        else { Data::DString(v_input.to_string()) };

        let mut text_obj = DataObject::new();
        text_obj.put_string("type", "text");
        text_obj.put_string("text", &ndata::Data::as_string(data_to_wrap));

        let mut content_arr = DataArray::new();
        content_arr.push_object(text_obj);

        let mut out = DataObject::new();
        out.put_array("content", content_arr);
        out
    }
}

fn make_error(msg: String) -> DataObject {
    let mut text = DataObject::new();
    text.put_string("type", "text");
    text.put_string("text", &msg);

    let mut content_arr = DataArray::new();
    content_arr.push_object(text);

    let mut out = DataObject::new();
    out.put_array("content", content_arr);
    out.put_boolean("isError", true);
    out
}
