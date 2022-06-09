let mut o = DataObject::new();
let file = File::open(path).unwrap();
let lines = io::BufReader::new(file).lines();
for line in lines {
  if let Ok(oneline) = line {
    if !oneline.starts_with("#") {
      let pair: Vec<_> = oneline.splitn(2, "=").collect();
      o.put_str(&pair[0], &pair[1]);
    }
  }
}
o