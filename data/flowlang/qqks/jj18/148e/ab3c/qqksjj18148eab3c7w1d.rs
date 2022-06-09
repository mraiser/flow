let v = serde_json::from_str(&a).unwrap();
DataObject::from_json(v)