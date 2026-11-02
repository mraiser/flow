//! The Python layer handles ndata containers only through handles
//! (DataObject / DataArray / DataBytes) and ndata's handle-taking methods -
//! never as bare references. A bare `Data::DObject(data_ref)` holds no
//! reference count, so passing one around after its last handle is dropped
//! lets a concurrent ndata GC free (or reuse) the slot first. This test fails
//! if raw-reference handling comes back into those files.

const FILES: &[&str] = &["src/pywrapper.rs", "src/pyenv.rs", "src/pycmd.rs"];

#[test]
fn python_layer_never_handles_raw_ndata_references() {
  let mut bad = Vec::new();
  for f in FILES {
    let src = std::fs::read_to_string(f).unwrap();
    for (n, line) in src.lines().enumerate() {
      let code = line.split("//").next().unwrap_or("");
      // Reading a handle's ref for display is fine; nothing else may touch it.
      let raw_ref = code.contains("data_ref") && !code.contains("format!(\"<NData");
      // Re-wrapping a bare ref, or taking the reference pop_property hands back.
      let rewrap = code.contains("DataObject::get(") || code.contains("DataArray::get(")
        || code.contains("DataBytes::get(") || code.contains("pop_property(");
      // Building a container Data by hand (matching on `(_)` is fine).
      let build = ["Data::DObject(", "Data::DArray(", "Data::DBytes("].iter()
        .any(|p| code.match_indices(p).any(|(i, _)| !code[i + p.len()..].starts_with("_)")));
      if raw_ref || rewrap || build {
        bad.push(format!("{}:{}: {}", f, n + 1, line.trim()));
      }
    }
  }
  assert!(bad.is_empty(), "raw ndata reference handling:\n{}", bad.join("\n"));
}
