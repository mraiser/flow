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
    appserver::init_globals();
    let cmd = Command::lookup(lib, control, command);
    let result = cmd.execute(arguments);

    match result {
        Ok(v)  => wrap_value(v),
        Err(e) => make_error(format!("{:?}", e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// helpers
// ─────────────────────────────────────────────────────────────────────────────

fn wrap_value(mut v: DataObject) -> DataObject {
    // You may return richer content arrays (images, file-links …) later.
    // For now we stringify whatever the flowlang command returned.
    let mut text = DataObject::new();
    text.put_string("type", "text");
    text.put_string("text", &v.to_string());

    let mut content_arr = DataArray::new();
    content_arr.push_object(text);

    let mut out = DataObject::new();
    out.put_array("content", content_arr);
    out
}

fn make_error(msg: String) -> DataObject {
    let mut text = DataObject::new();
    text.put_string("type", "text");
    text.put_string("text", &msg);

    let mut content_arr = DataArray::new();
    content_arr.push_object(text);

    let mut out = DataObject::new();
    out.put_array("content", content_arr);
    out.put_bool("isError", true);
    out