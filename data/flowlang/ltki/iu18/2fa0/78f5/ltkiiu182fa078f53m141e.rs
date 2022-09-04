let mut file = File::create(path).unwrap();
for (k,v) in data.objects() {
  let s = format!("{}={}\n",k,Data::as_string(v));
  file.write_all(s.as_bytes()).unwrap();
}
"OK".to_string()