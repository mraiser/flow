let v = serde_json::from_str(&a).unwrap();
DataArray::from_json(v)