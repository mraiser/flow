let mut a = DataObject::new();
a.put_str("body", &("<html><head><title>HELLO</title></head><body>Hello, world!<br><br>".to_string()+&path+"</body></html>"));
a.put_str("mimetype", "text/html");

a