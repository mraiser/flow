use ndata::dataobject::*;
use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;
use std::panic;
use std::fs;
use ndata::dataarray::*;
use std::sync::RwLock;
use state::Storage;
use std::sync::Once;
use std::net::TcpStream;
use ndata::heap::Heap;
use ndata::data::Data;

use crate::command::*;
use crate::datastore::*;
use crate::rfc2822date::*;

use crate::flowlang::http::hex_decode::hex_decode;
use crate::flowlang::system::time::time;
use crate::flowlang::file::mime_type::*;
pub fn execute(o: DataObject) -> DataObject {
let a0 = o.get_string("socket_address");
let a1 = o.get_string("library");
let a2 = o.get_string("control");
let a3 = o.get_string("command");
let ax = listen(a0, a1, a2, a3);
let mut o = DataObject::new();
o.put_string("a", &ax);
o
}

pub fn listen(socket_address:String, library:String, control:String, command:String) -> String {
START.call_once(|| {
  WEBSOCKS.set(RwLock::new(Heap::new()));
});

let listener = TcpListener::bind(socket_address).unwrap();
for stream in listener.incoming() {
  let cmd_path = [library.to_owned(), control.to_owned(), command.to_owned()];
  let mut stream = stream.unwrap();
  thread::spawn(move || {
    let remote_addr = stream.peer_addr().unwrap();
    let mut reader = stream.try_clone().unwrap();
    let mut line = read_line(&mut reader);
    let mut count = line.len();
    if count > 2 {
      line = (&line[0..count-2]).to_string();
      count = line.find(" ").unwrap();
      let method = (&line[0..count]).to_string();
      line = (&line[count+1..]).to_string();
      count = line.find(" ").unwrap();
      let protocol = (&line[count+1..]).to_string();
      let path = (&line[0..count]).to_string();

      let mut headers = DataObject::new();
      let mut last = "".to_string();
      loop {
        let line = read_line(&mut reader);
        let mut count = line.len();
        if count == 2 {
          break;
        }
        if (&line[0..1]).to_string() != " ".to_string(){
          count = line.find(":").unwrap();
          let mut key = (&line[0..count]).to_string();
          key = key.to_uppercase();
          let mut val = (&line[count+1..]).to_string();
          val = val.trim().to_string();
          if !headers.has(&key) {
            headers.put_string(&key, &val);
          }
          else {
            let d = headers.get_property(&key);
            if d.is_array() {
              d.array().push_string(&val);
            }
            else {
              let old = d.string();
              let mut v = DataArray::new();
              v.push_string(&old);
              v.push_string(&val);
              headers.put_array(&key, v);
            }
          }
          last = key;
        }
        else {
          let d = headers.get_property(&last);
          if d.is_array(){
            let mut v = d.array();
            let n = v.len() - 1;
            let mut old = v.get_string(n);
            v.remove_property(n);
            old = old + "\r\n" + line.trim_end();
            v.push_string(&old);
          }
          else {
            let mut old = d.string();
            old = old + "\r\n" + line.trim_end();
            headers.put_string(&last, &old);
          }
        }
      }

      let mut querystring = "".to_string();
      let mut params = DataObject::new();

      if method == "POST" {
        // extractPOSTParams
        let clstr = headers.get_string("CONTENT-LENGTH");
        let ctstr = headers.get_string("CONTENT-TYPE");
        let mut max = clstr.parse::<i64>().unwrap();

        let s = ctstr.to_lowercase();
        if s.starts_with("multipart/") {
          // MULTIPART

          panic!("No MIME MULTIPART support yet");


        }
        else {
          while max > 0 {
            let mut b = false;
            let mut buf = vec![];
            let n = read_until(&mut reader, b'=', &mut buf);
            max -= n as i64;
            let mut key = std::str::from_utf8(&buf).unwrap().to_string();
            if key.ends_with("=") {
              key = (&key[..n-1]).to_string();
            }

            buf = vec![];
            let n = read_until(&mut reader, b'&', &mut buf);
            max -= n as i64;
            let mut value = std::str::from_utf8(&buf).unwrap().to_string();
            if value.ends_with("&") {
              value = (&value[..n-1]).to_string();
            }
            else { b = true; }

            key = key.replace("+"," ");
            value = value.replace("+"," ");
            key = hex_decode(key);
            value = hex_decode(value);

            params.put_string(&key, &value);
            
            if b { break; }
          }
        }
      }
    
      let stream_id = &mut WEBSOCKS.get().write().unwrap().push((stream.try_clone().unwrap(), reader));

      let cmd:String;
      if path.contains("?"){
        let i = path.find("?").unwrap();
        cmd = path[0..i].to_string();
        querystring = path[i+1..].to_string();
        let mut oneline = querystring.to_owned();
        let mut oneparam:String;
        while oneline.len() > 0 {
          if oneline.contains("&")  {
            let i = oneline.find("&").unwrap();
            oneparam = oneline[0..i].to_string();
            oneline = oneline[i+1..].to_string();
          }
          else {
            oneparam = oneline;
            oneline = "".to_string();
          }

          if oneparam.contains("=") {
            let i = oneparam.find("=").unwrap();
            let key = oneparam[0..i].to_string();
            let value = oneparam[i+1..].to_string();
            params.put_string(&key, &value);
          }
        }
      }
      else {
        cmd = path;
      }
      let loc = remote_addr.to_string();
      headers.put_string("nn-userlocation", &loc);
      let mut request = DataObject::new();

      // FIXME - Is this necessary?
      if headers.has("ACCEPT-LANGUAGE"){ 
        let lang = headers.get_string("ACCEPT-LANGUAGE");
        request.put_string("language", &lang);
      }
      else {
        request.put_string("language", "*");
      }

      // FIXME - Is this necessary?
      if headers.has("HOST"){ 
        let h = headers.get_string("HOST");
        request.put_string("host", &h);
      }

      // FIXME - Is this necessary?
      if headers.has("REFERER"){ 
        let h = headers.get_string("REFERER");
        request.put_string("referer", &h);
      }

      request.put_string("protocol", &protocol);
      request.put_string("path", &cmd);
      request.put_string("loc", &loc);
      request.put_string("method", &method);
      request.put_string("querystring", &querystring);
      request.put_object("headers", headers.clone());
      request.put_object("params", params);
      request.put_int("timestamp", time());
      request.put_int("stream_id", *stream_id as i64);

      // FIXME
  //		CONTAINER.getDefault().fireEvent("HTTP_BEGIN", log);

      // FIXME - implement or remove
      if headers.has("TRANSFER-ENCODING"){
        let trenc = headers.get_string("TRANSFER-ENCODING");
        if trenc.to_uppercase() == "CHUNKED" {
          // CHUNKED
        }
      }

      
      // FIXME - Implement keep-alive
//      let mut ka = "close".to_string();
//      if headers.has("CONNECTION") { ka = headers.get_string("CONNECTION"); }

      // FIXME - origin is never used
//      let mut origin = "null".to_string();
//      if headers.has("ORIGIN") { origin = headers.get_string("ORIGIN"); }

      // FIXME
//			setRequestParameters(params);

      let command = Command::lookup(&cmd_path[0], &cmd_path[1], &cmd_path[2]);
      let mut response = DataObject::new();
      let dataref = response.data_ref;

      let result = panic::catch_unwind(|| {
        let mut p = DataObject::get(dataref);
        let o = command.execute(request).unwrap();
        p.put_object("a", o);
      });
      
      match result {
        Ok(_x) => (),
        Err(e) => {
          
          let s = match e.downcast::<String>() {
            Ok(panic_msg) => format!("{}", panic_msg),
            Err(_) => "unknown error".to_string()
          };        
          
          let mut o = DataObject::new();
          let s = format!("<html><head><title>500 - Server Error</title></head><body><h2>500</h2>Server Error: {}</body></html>", s);
          o.put_string("body", &s);
          o.put_int("code", 500);
          o.put_string("mimetype", "text/html");
          response.put_object("a", o);
        }
      }

      WEBSOCKS.get().write().unwrap().decr(*stream_id);
        
      if !headers.has("SEC-WEBSOCKET-KEY") {
        let response = response.get_object("a").clone();

        let body:String;
        let mimetype:String;
        let len:i64;
        let code:u16;
        let msg:String;
        let mut headers:DataObject;
        
        let isfile = response.has("file") && response.get_property("file").is_string();
        
        if isfile { body = response.get_string("file"); }
        else if response.has("body") && response.get_property("body").is_string() { body = response.get_string("body"); }
        else { body = "".to_owned(); }

        if response.has("code") && response.get_property("code").is_int() { code = response.get_int("code") as u16; }
        else { code = 200; }

        if response.has("msg") && response.get_property("msg").is_string() { msg = response.get_string("msg"); }
        else { 
          if code < 200 { msg = "INFO".to_string(); }
          else if code < 300 { msg = "OK".to_string(); }
          else if code < 400 { msg = "REDIRECT".to_string(); }
          else if code < 500 { msg = "CLIENT ERROR".to_string(); }
          else { msg = "SERVER ERROR".to_string(); }
        }

        if response.has("headers") && response.get_property("headers").is_object() { headers = response.get_object("headers"); }
        else { headers = DataObject::new(); }

        if response.has("mimetype") && response.get_property("mimetype").is_string() { mimetype = response.get_string("mimetype"); }
        else if headers.has("Content-Type") { mimetype = headers.get_string("Content-Type"); }
        else if isfile { mimetype = mime_type(cmd); }
        else { mimetype = "text/plain".to_string(); }

        if response.has("len") && response.get_property("len").is_int() { len = response.get_int("len"); }
        else if headers.has("Content-Length") { len = headers.get_int("Content-Length"); }
        else if isfile { len = fs::metadata(&body).unwrap().len() as i64; }
        else { len = body.len() as i64; }

        //FIXME
  //		int[] range = extractRange(len, h);
  //		if (range[1] != -1) len = range[1] - range[0] + 1;
  //		String res = range[0] == -1 ? "200 OK" : "206 Partial Content";

        let date = RFC2822Date::now().to_string();

        headers.put_string("Date", &date);
        headers.put_string("Content-Type", &mimetype);
        if len != -1 { headers.put_string("Content-Length", &len.to_string()); }
        // FIXME
  //      if (acceptRanges != null) h.put("Accept-Ranges", acceptRanges);
  //      if (range != null && range[0] != -1) h.put("Content-Range","bytes "+range[0]+"-"+range[1]+"/"+range[2]);
  //      if (expires != -1) h.put("Expires", toHTTPDate(new Date(expires)));

  //      let later = now.add(Duration::weeks(52));
  //      let cookie = "sessionid=".to_string()+&sid+"; Path=/; Expires="+&later.to_rfc2822();
  //      headers.put_string("Set-Cookie", &cookie);

        // FIXME
  //		if (origin != null)
  //		{
  //			String cors = getCORS(name, origin);
  //			if (cors != null)
  //			{
  //				h.put("Access-Control-Allow-Origin", cors);
  //				if (!cors.equals("*")) h.put("Vary", "Origin");
  //			}
  //		}

        let mut reshead = "HTTP/1.1 ".to_string()+&code.to_string()+" "+&msg+"\r\n";
        for (k,v) in headers.objects() {
          reshead = reshead +&k + ": "+&Data::as_string(v)+"\r\n";
        }
        reshead = reshead + "\r\n";
        
        if isfile {
          stream.write(reshead.as_bytes()).unwrap();
          let mut file = fs::File::open(&body).unwrap();
          let chunk_size = 0x4000;
          loop {
            let mut chunk = Vec::with_capacity(chunk_size);
            let n = std::io::Read::by_ref(&mut file).take(chunk_size as u64).read_to_end(&mut chunk).unwrap();
            if n == 0 { break; }
            stream.write(&chunk).unwrap();
            if n < chunk_size { break; }
          }
        }
        else {
          let response = reshead + &body;
          //println!("{}\r\n", &response);
          stream.write(response.as_bytes()).unwrap();
        }
        stream.flush().unwrap();
      }
        // FIXME
  //				clearRequestParameters();

      
    }
    // FIXME
//				CONTAINER.getDefault().fireEvent("HTTP_END", log);

    DataStore::gc();
  });
}
"OK".to_string()
}

static START: Once = Once::new();
pub static WEBSOCKS:Storage<RwLock<Heap<(TcpStream, TcpStream)>>> = Storage::new();

fn read_line(reader: &mut TcpStream) -> String {
  let mut buf = [0];
  let mut line: String = "".to_string();
  loop {
    let res = reader.read_exact(&mut buf);
    if res.is_err() { break; }
    line = line + &std::str::from_utf8(&buf).unwrap();
    if buf[0] == b'\r' {
      let res = reader.read_exact(&mut buf);
      if res.is_err() { break; }
      line = line + std::str::from_utf8(&buf).unwrap();
      if buf[0] == b'\n' {
        break;
      }
    }
    if line.len() >= 4096 { break; } // FIXME - What is an appropriate max HTTP request line length?
  }
  line
}

fn read_until(reader: &mut TcpStream, c: u8, bufout: &mut Vec<u8>) -> usize {
  let mut buf = [0];
  let mut i = 0;
  loop {
    let res = reader.read_exact(&mut buf);
    if res.is_err() { break; }
    i += 1;
    bufout.push(buf[0]);
    if buf[0] == c {
      break;
    }
    if i >= 4096 { break; } // FIXME - What is an appropriate max HTTP request line length?
  }
  i
}

