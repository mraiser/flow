use ndata::dataobject::*;
use std::io::prelude::*;
use std::net::TcpListener;
use std::thread;
use std::panic;
use std::fs;
use std::path::Path;
use std::fs::metadata;
use std::fs::create_dir_all;
use ndata::dataarray::*;
use std::net::TcpStream;
use std::time::Duration;
use ndata::data::Data;

use crate::command::*;
use crate::datastore::*;
use crate::rfc2822date::*;
use crate::sha1::*;
use crate::base64::*;

use crate::generated::flowlang::http::hex_decode::hex_decode;
use crate::generated::flowlang::system::time::time;
use crate::generated::flowlang::file::mime_type::mime_type;
use crate::generated::flowlang::system::unique_session_id::unique_session_id;
use crate::generated::flowlang::object::index_of::index_of;
use crate::generated::flowlang::file::read_properties::read_properties;
use crate::generated::flowlang::file::write_properties::write_properties;

// FIXME - The code in this file makes the assumption in several places that the process was launched from the root directory. That assumption should only be made once, in the event that no root directory is specified, by whatever initializes the flowlang DataStore.

pub fn run() {
  let _system = init_globals();
  
  // Start Timers
  thread::spawn(timer_loop);
  
  // Start HTTP
  thread::spawn(http_listen);
  
  // FIXME - Check sessions
  loop {
    let dur = Duration::from_millis(5000);
    thread::sleep(dur);
  }
}

pub fn http_listen() {
  let system = DataStore::globals().get_object("system");
  let socket_address = system.get_object("config").get_string("socket_address");

  let listener = TcpListener::bind(socket_address).unwrap();
  for stream in listener.incoming() {
    let mut stream = stream.unwrap();
    thread::spawn(move || {
      let remote_addr = stream.peer_addr().unwrap();
      let mut line = read_line(&mut stream);
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
          let line = read_line(&mut stream);
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
              headers.put_str(&key, &val);
            }
            else {
              let d = headers.get_property(&key);
              if d.is_array() {
                d.array().push_str(&val);
              }
              else {
                let old = d.string();
                let mut v = DataArray::new();
                v.push_str(&old);
                v.push_str(&val);
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
              v.push_str(&old);
            }
            else {
              let mut old = d.string();
              old = old + "\r\n" + line.trim_end();
              headers.put_str(&last, &old);
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
              let n = read_until(&mut stream, b'=', &mut buf);
              max -= n as i64;
              let mut key = std::str::from_utf8(&buf).unwrap().to_string();
              if key.ends_with("=") {
                key = (&key[..n-1]).to_string();
              }

              buf = vec![];
              let n = read_until(&mut stream, b'&', &mut buf);
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

              params.put_str(&key, &value);
              
              if b { break; }
            }
          }
        }
        
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
              let key = hex_decode(oneparam[0..i].to_string());
              let value = hex_decode(oneparam[i+1..].to_string());
              params.put_str(&key, &value);
            }
          }
        }
        else {
          cmd = path;
        }
        let loc = remote_addr.to_string();
        headers.put_str("nn-userlocation", &loc);
        let mut request = DataObject::new();

        // FIXME - Is this necessary?
        if headers.has("ACCEPT-LANGUAGE"){ 
          let lang = headers.get_string("ACCEPT-LANGUAGE");
          request.put_str("language", &lang);
        }
        else {
          request.put_str("language", "*");
        }

        // FIXME - Is this necessary?
        if headers.has("HOST"){ 
          let h = headers.get_string("HOST");
          request.put_str("host", &h);
        }

        // FIXME - Is this necessary?
        if headers.has("REFERER"){ 
          let h = headers.get_string("REFERER");
          request.put_str("referer", &h);
        }

        request.put_str("protocol", &protocol);
        request.put_str("path", &cmd);
        request.put_str("loc", &loc);
        request.put_str("method", &method);
        request.put_str("querystring", &querystring);
        request.put_object("headers", headers.duplicate());
        request.put_object("params", params);
        
        let now = time();
        request.put_i64("timestamp", now);

        fire_event("app", "HTTP_BEGIN", request.duplicate());

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

        // FIXME - origin is never used, impliment CORS
  //      let mut origin = "null".to_string();
  //      if headers.has("ORIGIN") { origin = headers.get_string("ORIGIN"); }

        let mut response = DataObject::new();
        let dataref = response.data_ref;

        let result = panic::catch_unwind(|| {
          let mut p = DataObject::get(dataref);
          let o = handle_request(request.duplicate(), stream.try_clone().unwrap());
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
            o.put_str("body", &s);
            o.put_i64("code", 500);
            o.put_str("mimetype", "text/html");
            response.put_object("a", o);
          }
        }
          
        if headers.has("SEC-WEBSOCKET-KEY") { fire_event("app", "WEBSOCK_END", request.duplicate()); }
        else {
          let response = response.get_object("a").duplicate();

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

          if response.has("code") && response.get_property("code").is_int() { code = response.get_i64("code") as u16; }
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

          if response.has("len") && response.get_property("len").is_int() { len = response.get_i64("len"); }
          else if headers.has("Content-Length") { len = headers.get_i64("Content-Length"); }
          else if isfile { len = fs::metadata(&body).unwrap().len() as i64; }
          else { len = body.len() as i64; }

          //FIXME
    //		int[] range = extractRange(len, h);
    //		if (range[1] != -1) len = range[1] - range[0] + 1;
    //		String res = range[0] == -1 ? "200 OK" : "206 Partial Content";

          let date = RFC2822Date::new(now).to_string();

          headers.put_str("Date", &date);
          headers.put_str("Content-Type", &mimetype);
          if len != -1 { headers.put_str("Content-Length", &len.to_string()); }
          // FIXME
    //      if (acceptRanges != null) h.put("Accept-Ranges", acceptRanges);
    //      if (range != null && range[0] != -1) h.put("Content-Range","bytes "+range[0]+"-"+range[1]+"/"+range[2]);
    //      if (expires != -1) h.put("Expires", toHTTPDate(new Date(expires)));

          let session_id = request.get_object("session").get_string("id");
          let later = now + 31536000000; //system.get_object("config").get_i64("sessiontimeoutmillis");
          let cookie = "sessionid=".to_string()+&session_id+"; Path=/; Expires="+&RFC2822Date::new(later).to_string();
          headers.put_str("Set-Cookie", &cookie);

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
            stream.write(response.as_bytes()).unwrap();
          }
    			
    			fire_event("app", "HTTP_END", response);
    			
          stream.flush().unwrap();
        }
      }

      DataStore::gc();
    });
  }
}

fn handle_request(mut request: DataObject, mut stream: TcpStream) -> DataObject {
  let system = DataStore::globals().get_object("system");

  let path = hex_decode(request.get_string("path"));
  let mut params = request.get_object("params");
  let mut headers = request.get_object("headers");
    
  let mut session_id = "".to_string();
  if params.has("session_id") { session_id = params.get_string("session_id"); }
  else {
    if headers.has("COOKIE") {
      let cookies = headers.get_string("COOKIE");
      let sa = cookies.split(";");
      for cookie in sa {
        let cookie = cookie.trim();
        if cookie.starts_with("sessionid="){
          session_id = cookie[10..].to_string();
          break;
        }
      }
    }
  }
  if session_id == "" { session_id = unique_session_id(); }
//  println!("{}", session_id);
  let mut sessions = system.get_object("sessions");
  let mut session;
  if !sessions.has(&session_id) {
    session = DataObject::new();
    session.put_i64("count", 0);
    session.put_str("id", &session_id);

    // FIXME - Replace this with alt login scheme
    let file = DataStore::new().root
                .parent().unwrap()
                .join("runtime")
                .join("securitybot")
                .join("session.properties");
    let mut b = false;
    if file.exists() {
      let p = read_properties(file.to_owned().into_os_string().into_string().unwrap());
      if p.has(&session_id) { 
        let r = p.get_string(&session_id); 
        let user = get_user(&r);
        if user.is_some(){
          let user = user.unwrap();
          session.put_str("username", &r);
          session.put_object("user", user);
          b = true;
        }
      }
    }
    
    if !b {
      let mut user = DataObject::new();
      user.put_str("displayname", "Anonymous");
      user.put_array("groups", DataArray::new());
      user.put_str("username", "anonymous");
      session.put_str("username", "anonymous");
      session.put_object("user", user);
    }
    
    sessions.put_object(&session_id, session.duplicate());
  }
  else {
    session = sessions.get_object(&session_id);
  }
  
  let expire = time() + system.get_object("config").get_i64("sessiontimeoutmillis");
  session.put_i64("expire", expire);
  
  let count = session.get_i64("count") + 1;
  session.put_i64("count", count);
  
  if session.has("user") {
    headers.put_str("nn-username", &session.get_string("username"));
    let groups = session.get_object("user").get_array("groups");
    headers.put_array("nn-groups", groups);
  }
  else {
    headers.put_str("nn-username", "anonymous");
    headers.put_str("nn-groups", "anonymous");
  }
  
  request.put_object("session", session);
  request.put_str("sessionid", &session_id);
  
  let mut res = DataObject::new();
  
  let mut p = "html".to_string() + &path;
  let mut b = false;
  
  if Path::new(&p).exists() {
    let md = metadata(&p).unwrap();
    if md.is_dir() {
      if !p.ends_with("/") { p += "/"; }
      p += "index.html";
      if Path::new(&p).exists() { b = true; }
    }
    else { b = true; }
  }
  
  if b {
    res.put_str("file", &p);
    res.put_str("mimetype", &mime_type(p));
  }
  else if path == "/" {
    let default_app = system.get_string("default_app");
    let p = "/".to_owned()+&default_app+"/index.html";
    res.put_i64("code", 302);
    res.put_str("msg", "FOUND");
    let mut h = DataObject::new();
    h.put_str("Location", &p);
    res.put_object("headers", h);
  }
  else {
    if headers.has("SEC-WEBSOCKET-KEY") {
      b = true;
      
      let key = headers.get_string("SEC-WEBSOCKET-KEY");
      let key = key.trim();
      let key = key.to_string()+"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
      let mut checksum = SHA1::new();
      let _hash = checksum.update(&key);
      let hash = checksum.finish();
      let key2: String = Base64::encode(hash).into_iter().collect();
      let mut response = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n".to_string();
      response += "Sec-WebSocket-Accept: ";
      response += key2.trim();
      response += "\r\n";
      response += "Sec-WebSocket-Protocol: newbound\r\n\r\n";
      stream.write(response.as_bytes()).unwrap();
      
      fire_event("app", "WEBSOCK_BEGIN", request.duplicate());
      
      loop {
        if !system.get_bool("running") { break; }
        
        let base:i64 = 2;
        let pow7 = base.pow(7);
        let mut lastopcode = 0;
        let mut baos: Vec<u8> = Vec::new();

        loop {
          let mut buf = [0; 1];
          let _ = stream.read_exact(&mut buf).unwrap();
          let i = buf[0] as i64;
          let fin = (pow7 & i) != 0;
          let rsv1 = (base.pow(6) & i) != 0;
          let rsv2 = (base.pow(5) & i) != 0;
          let rsv3 = (base.pow(4) & i) != 0;

          if rsv1 || rsv2 || rsv3 { panic!("Websocket failed - Unimplimented"); } 

          let mut opcode = 0xf & i;

          let _ = stream.read_exact(&mut buf).unwrap();
          let i = buf[0] as i64;
          let mask = (pow7 & i) != 0;
          if !mask { panic!("Websocket failed - Mask required"); } 

          let mut len = i - pow7;

          if len == 126 {
            let mut buf = [0; 2];
            let _ = stream.read_exact(&mut buf).unwrap();
            len = (buf[0] as i64 & 0x000000FF) << 8;
            len += buf[1] as i64 & 0x000000FF;
          }
          else if len == 127 {
            let mut buf = [0; 8];
            let _ = stream.read_exact(&mut buf).unwrap();
            len = (buf[0] as i64 & 0x000000FF) << 56;
            len += (buf[1] as i64 & 0x000000FF) << 48;
            len += (buf[2] as i64 & 0x000000FF) << 40;
            len += (buf[3] as i64 & 0x000000FF) << 32;
            len += (buf[4] as i64 & 0x000000FF) << 24;
            len += (buf[5] as i64 & 0x000000FF) << 16;
            len += (buf[6] as i64 & 0x000000FF) << 8;
            len += buf[7] as i64 & 0x000000FF;
          }

          // FIXME - Should read larger messages in chunks
          // if len > 4096 { panic!("Websocket message too long ({})", len); } 
          let len = len as usize;

          let mut maskkey = [0; 4];
          let _ = stream.read_exact(&mut maskkey).unwrap();

          let mut buf = vec![0; len as usize];
          let _ = stream.read_exact(&mut buf).unwrap();
          let mut i:usize = 0;
          while i < len {
            buf[i] = buf[i] ^ maskkey[i % 4];
            i += 1;
          }
          
          baos.append(&mut buf);

          if opcode == 0 {
            println!("continuation frame");
          }
          else if opcode == 1 || opcode == 2 { lastopcode = opcode; }
          else if opcode == 8 {  return DataObject::new(); } // panic!("Websocket closed"); } 
          else if opcode == 9 {
            println!("ping");
          }
          else if opcode == 10 {
            println!("pong");
          }
          else {
            println!("UNEXPECTED OPCODE: {}", opcode);
          }

          if fin {
            if opcode == 0 {
              opcode = lastopcode;
            }
            
            if opcode == 1 {
              // text frame
              break;
            }
            else if opcode == 2 {
              // binary frame
              // FIXME - passing text anyway.
              break;
            }
          }
        }

        let msg = std::str::from_utf8(&baos).unwrap().to_owned();        
        
        let system = system.duplicate();
        let mut stream = stream.try_clone().unwrap();
        let request = request.duplicate();
        
        let sid = session_id.to_owned();
        thread::spawn(move || {
          if msg.starts_with("cmd ") {
            let msg = &msg[4..];
            let d = DataObject::from_string(msg);
            let app = d.get_string("bot");
            let cmd = d.get_string("cmd");
            let pid = d.get_string("pid");
            let mut params = d.get_object("params");
            
            for (k,v) in request.objects() {
              if k != "params" {
                params.set_property(&("nn_".to_string()+&k), v);
              }
            }
            
            let (b, ctldb, id) = lookup_command_id(system.duplicate(), app, cmd.to_owned());
            
            let mut o;
            if b {
              let command = Command::new(&ctldb, &id);
              if check_security(&command, &sid) {
                command.cast_params(params.duplicate());
                
                let response = DataObject::new();
                let dataref = response.data_ref;

                let result = panic::catch_unwind(|| {
                  let mut p = DataObject::get(dataref);
                  let o = command.execute(params).unwrap();
                  p.put_object("a", o);
                });
                
                match result {
                  Ok(_x) => {
                    let oo = response.get_object("a");
                    o = format_result(command, oo);
                  },
                  Err(e) => {
                    let msg = match e.downcast::<String>() {
                      Ok(panic_msg) => format!("{}", panic_msg),
                      Err(_) => "unknown error".to_string()
                    };        
                    o = DataObject::new();
                    o.put_str("status", "err");
                    o.put_str("msg", &msg);
                  },
                }
              }
              else {
                o = DataObject::new();
                o.put_str("status", "err");
                let err = format!("UNAUTHORIZED: {}", &cmd);
                o.put_str("msg", &err);
              }
            }
            else {
              o = DataObject::new();
              o.put_str("status", "err");
              let err = format!("Unknown websocket command: {}", &cmd);
              o.put_str("msg", &err);
            }
            
            if !o.has("status") { o.put_str("status", "ok"); }
            o.put_str("pid", &pid);
            
            let msg = o.to_string();
            let msg = msg.as_bytes();
            
            let n = msg.len() as i64;
            let mut reply: Vec<u8> = Vec::new();

            reply.push(129); // Text = 129 / Binary = 130;

            if n < 126 {
              reply.push((n & 0xFF) as u8);
            }
            else if n < 65536 {
              reply.push(126);
              reply.push(((n >> 8) & 0xFF) as u8);
              reply.push((n & 0xFF) as u8);
            }
            else {
              reply.push(127);
              reply.push(((n >> 56) & 0xFF) as u8);
              reply.push(((n >> 48) & 0xFF) as u8);
              reply.push(((n >> 40) & 0xFF) as u8);
              reply.push(((n >> 32) & 0xFF) as u8);
              reply.push(((n >> 24) & 0xFF) as u8);
              reply.push(((n >> 16) & 0xFF) as u8);
              reply.push(((n >> 8) & 0xFF) as u8);
              reply.push((n & 0xFF) as u8);
            }

            reply.extend_from_slice(msg);

            let _ = stream.write(&reply).unwrap();
          }
        });
      }
    }
    else {
      // Not a websocket, try app
      let mut sa = path.split("/");
      let appname = sa.nth(1).unwrap().to_string();
      let apps = system.get_object("apps");
      if apps.has(&appname) {
        let app = apps.get_object(&appname);
        request.put_object("app", app);
        let mut a = DataArray::new();
        for mut s in sa {
          if s == "" { s = "index.html"; }
          a.push_str(s);
        }
        if a.len() == 0 { a.push_str("index.html"); }
        
        let cmd = a.get_string(0);
        let mut path = cmd.to_owned();
        a.remove_property(0);
        for p in a.objects() {
          path += "/";
          path += &p.string();
        }
        
        // try app html dir
        let mut p = "runtime/".to_string()+&appname+"/html/"+&path;

        if Path::new(&p).exists() {
          let md = metadata(&p).unwrap();
          if md.is_dir() {
            if !p.ends_with("/") { p += "/"; }
            p += "index.html";
            if Path::new(&p).exists() { b = true; }
          }
          else { b = true; }
        }
        
        if b {
          res.put_str("file", &p);
          res.put_str("mimetype", &mime_type(p));
        }
        else {
          // try app src dir
          let mut p = "runtime/".to_string()+&appname+"/src/html/"+&appname+"/"+&path;

          if Path::new(&p).exists() {
            let md = metadata(&p).unwrap();
            if md.is_dir() {
              if !p.ends_with("/") { p += "/"; }
              p += "index.html";
              if Path::new(&p).exists() { b = true; }
            }
            else { b = true; }
          }
          
          if b {
            res.put_str("file", &p);
            res.put_str("mimetype", &mime_type(p));
          }
          else {
            // try app command
            let (bb, ctldb, id) = lookup_command_id(system.duplicate(), appname, cmd.to_owned());
            if bb {
              b = true;
              
              let command = Command::new(&ctldb, &id);
              for (k,v) in request.objects() {
                if k != "params" {
                  params.set_property(&("nn_".to_string()+&k), v);
                }
              }
              //println!("{}", params.to_string());
              command.cast_params(params.duplicate());
              
              if check_security(&command, &(session_id.to_owned())) {
                let r = command.return_type.to_owned();
                let o = command.execute(params.duplicate()).unwrap();
                let d = format_result(command, o);
                if r == "File" {
                  res.put_str("file", &d.get_string("data"));
                  res.put_str("mimetype", &mime_type(p));
                }
                else {
                  let s;
                  if params.has("callback") {
                    s = params.get_string("callback") + "(" + &d.to_string() + ")";
                  }
                  else {
                    s = d.to_string();
                  }
                  res.put_str("body", &s);
                  res.put_str("mimetype", "application/json");
                }
              }
              else {
                let mut o = DataObject::new();
                o.put_str("status", "err");
                let err = format!("UNAUTHORIZED: {}", &cmd);
                o.put_str("msg", &err);
                res.put_str("body", &o.to_string());
                res.put_str("mimetype", "application/json");
              }
            }
          }
        }
      }
    }
    
    if !b {
      // 404
      let p = "html/404.html";
      if Path::new(&p).exists() {
        res.put_str("file", &p);
        res.put_str("mimetype", "text/html");
      }
      res.put_i64("code", 404);
      res.put_str("msg", "NOT FOUND");
    }
  }

  res
}

pub fn check_auth(lib:&str, id:&str, session_id:&str, write:bool) -> bool {
  let store = DataStore::new();
  let system = DataStore::globals().get_object("system");
  
  if !system.get_object("config").get_bool("security") { 
    return true; 
  }
  
  let libdata = system.get_object("libraries").get_object(lib);
  let libgroups = libdata.get_property("readers");
  
  let which;
  if write { which = "writers"; }
  else { which = "readers"; }
  
  let ogroups;
  if !store.get_data_file(lib, id).exists() {
    ogroups = libgroups.clone();
  }
  else {
    let data = store.get_data(lib, id);
    let o;
    if data.has(which) { o = data.get_array(which); }
    else { o = DataArray::new(); }
    ogroups = Data::DArray(o.data_ref);
  }
    
  if index_of(libgroups.clone(), Data::DString("anonymous".to_string())) != -1 {
    if index_of(ogroups.clone(), Data::DString("anonymous".to_string())) != -1 {
      return true;
    }
  }

  let sessions = system.get_object("sessions");
  let groups;
  if sessions.has(session_id) {
    let session = sessions.get_object(session_id);
    let user = session.get_object("user");
    groups = user.get_array("groups");
  }
  else { groups = DataArray::new(); }
  
  if index_of(Data::DArray(groups.data_ref), Data::DString("admin".to_string())) != -1 {
    return true;
  }
  
  for g in groups.objects() {
    if index_of(libgroups.clone(), g.clone()) != -1 {
      if index_of(ogroups.clone(), g.clone()) != -1 {
        return true;
      }
    }
  }
    
  false
}

pub fn check_security(command:&Command, session_id:&str) -> bool {
//  println!("session id: {}", session_id);
  let system = DataStore::globals().get_object("system");
  
  if !system.get_object("config").get_bool("security") { 
    return true; 
  }
    
  let lib = system.get_object("libraries").get_object(&command.lib);
  
  let libgroups = lib.get_property("readers");
  let cmdgroups = &command.readers;
  if index_of(libgroups.clone(), Data::DString("anonymous".to_string())) != -1 {
    if cmdgroups.iter().position(|r| r == "anonymous").is_some() {
      return true;
    }
  }
  
  let sessions = system.get_object("sessions");
  let groups;
  if sessions.has(session_id) {
    let session = sessions.get_object(session_id);
    let user = session.get_object("user");
    groups = user.get_array("groups");
  }
  else { groups = DataArray::new(); }
  
  if index_of(Data::DArray(groups.data_ref), Data::DString("admin".to_string())) != -1 {
    return true;
  }
  
  for g in groups.objects() {
    if index_of(libgroups.clone(), g.clone()) != -1 {
      if cmdgroups.iter().position(|r| r == &(g.string())).is_some() {
        return true;
      }
    }
  }
    
  false
}

pub fn log_in(sessionid:&str, username:&str, password:&str) -> bool {
  let user = get_user(username);
  let mut e = DataObject::new();
  e.put_str("user", username);
  e.put_str("sessionid", sessionid);
  if user.is_some() {
    let user = user.unwrap();
    if user.get_string("password") == password {
      let system = DataStore::globals().get_object("system");
      let sessions = system.get_object("sessions");
      let mut session = sessions.get_object(sessionid);
      session.put_str("username", username);
      session.put_object("user", user);
      
      fire_event("security", "LOGIN", e);

      return true;
    }
  }

  fire_event("security", "LOGIN_FAIL", e);
  
  false
}

pub fn remove_timer(tid:&str) -> bool {
  let system = DataStore::globals().get_object("system");
  let mut timers = system.get_object("timers");
  if timers.has(tid) {
    timers.remove_property(tid);
    return true;
  }
  false
}

pub fn add_timer(tid:&str, mut tdata:DataObject) {
  let system = DataStore::globals().get_object("system");
  let mut timers = system.get_object("timers");    
  let start = tdata.get_i64("start");
  let start = to_millis(start, tdata.get_string("startunit"));
  let interval = tdata.get_i64("interval");
  let interval = to_millis(interval, tdata.get_string("intervalunit"));
  tdata.put_i64("startmillis", start);
  tdata.put_i64("intervalmillis", interval);
  timers.put_object(&tid, tdata);
}

pub fn remove_event_listener(id:&str) -> bool {
  let mut b = false;
  let system = DataStore::globals().get_object("system");
  let events = system.get_object("events");
  for (_k1, v1) in events.objects(){
    for (_k2, v2) in v1.object().objects(){
      for k3 in v2.object().keys(){
        if k3 == id { 
          v2.object().remove_property(&k3); 
          b = true;
        }
      }
    }
  }
  b
}

pub fn add_event_listener(id:&str, app:&str, event:&str, cmdlib:&str, cmdid:&str) {
  //println!("Adding event listener {}, {}, {}, {}, {}", id, app, event, cmdlib, cmdid);
  let system = DataStore::globals().get_object("system");
  let mut events = system.get_object("events");
  let mut bot;
  let mut list;
  if events.has(app) {
    bot = events.get_object(app);
  }
  else {
    bot = DataObject::new();
    events.put_object(app, bot.duplicate());
  }
  if bot.has(event) {
    list = bot.get_object(event);
  }
  else {
    list = DataObject::new();
    bot.put_object(event, list.duplicate());
  }
  let mut cmd = DataObject::new();
  cmd.put_str("lib", cmdlib);
  cmd.put_str("cmd", cmdid);
  list.put_object(id, cmd);
}

pub fn fire_event(app:&str, event:&str, data:DataObject) {
  let system = DataStore::globals().get_object("system");
  let mut events = system.get_object("events");
  if !events.has(app) { events.put_object(app, DataObject::new()); }
  let mut bot = events.get_object(app);
  if !bot.has(event) { bot.put_object(event, DataObject::new()); }
  else {
    let list = bot.get_object(event);
    for (_, e) in list.objects() {
      let e = e.object();
      let lib = e.get_string("lib");
      let id = e.get_string("cmd");
      let command = Command::new(&lib, &id);
      let  _ = command.execute(data.duplicate());
    }
  }
  
}

fn timer_loop() {
  let system = DataStore::globals().get_object("system");
  loop {
    if system.get_bool("running") {
      let now = time();
      let mut timers = system.get_object("timers");
      for (id, timer) in timers.objects() {
        let mut timer = timer.object();
        let when = timer.get_i64("startmillis");
        if when <= now {
          timers.remove_property(&id);
          let cmdid = timer.get_string("cmd");
          let params = timer.get_object("params");
          let repeat = timer.get_bool("repeat");
          let db = timer.get_string("cmddb");
          let mut ts = timers.duplicate();
          thread::spawn(move || {
            let cmd = Command::new(&db, &cmdid);
            let _x = cmd.execute(params).unwrap();
            
            if repeat {
              let next = now + timer.get_i64("intervalmillis");
              timer.put_i64("startmillis", next);
              ts.put_object(&id, timer);
            }
          });
        }
      }
    }
    else {
      break;
    }

    let dur = Duration::from_millis(1000);
    thread::sleep(dur);
  }
}

pub fn load_library(j:&str) {
  let store = DataStore::new();
  let system = DataStore::globals().get_object("system");
  let path = store.root.join(j).join("meta.json");
  let s = fs::read_to_string(&path).unwrap();
  let mut o2 = DataObject::from_string(&s);
  
  let mut readers = DataArray::new();
  let mut writers = DataArray::new();
  if o2.has("readers") { 
    for r in o2.get_array("readers").objects() { readers.push_str(&(r.string())); }
  }
  if o2.has("writers") { 
    for w in o2.get_array("writers").objects() { writers.push_str(&(w.string())); }
  }
  o2.put_array("readers", readers);
  o2.put_array("writers", writers);
  o2.put_str("id", j);

  let mut libraries = system.get_object("libraries");
  libraries.put_object(j, o2);
}

pub fn get_user(username:&str) -> Option<DataObject> {
  let system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let users = system.get_object("users");
    if users.has(username) {
      return Some(users.get_object(username));
    }
  }
  None
}

pub fn delete_user(username:&str) -> bool{
  let system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let mut users = system.get_object("users");
    if users.has(&username) {
      users.remove_property(&username);
      let root = DataStore::new().root.parent().unwrap().join("users");
      let propfile = root.join(&(username.to_owned()+".properties"));
      let x = fs::remove_file(propfile);
      if x.is_ok() { return true; }
    }
  }
  false
}

pub fn set_user(username:&str, user:DataObject) {
  let system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let mut users = system.get_object("users");
    if users.has(&username) {
      let mut u2 = users.get_object(&username);
      for (k,v) in user.objects() { u2.set_property(&k, v); }
    }
    else { users.put_object(username, user.duplicate()); }
    
    let mut user = user.deep_copy();
    let groups = user.get_array("groups");
    let mut s = "".to_string();
    for g in groups.objects() {
      let g = g.string();
      if s != "" { s += ","; }
      s += &g
    }
    user.put_str("groups", &s);
    let root = DataStore::new().root.parent().unwrap().join("users");
    let propfile = root.join(&(username.to_owned()+".properties"));
    write_properties(propfile.into_os_string().into_string().unwrap(), user);
  }
}

pub fn load_users() {
  let mut system = DataStore::globals().get_object("system");
  if system.get_object("config").get_bool("security") { 
    let mut users;
    let mut b = false;
    if system.has("users") { users = system.get_object("users"); }
    else {
      b = true;
      users = DataObject::new();
    }
    
    let root = DataStore::new().root.parent().unwrap().join("users");
    let propfile = root.join("admin.properties");
    if !propfile.exists() {
      let _x = create_dir_all(&root);
      let mut admin = DataObject::new();
      admin.put_str("displayname", "System Administrator");
      admin.put_str("groups", "admin");
      admin.put_str("password", &unique_session_id());
      write_properties(propfile.into_os_string().into_string().unwrap(), admin);
    }
    
    for file in fs::read_dir(&root).unwrap() {
      let file = file.unwrap();
      let name = file.file_name().into_string().unwrap();
      if name.ends_with(".properties") {
        let mut user = read_properties(file.path().into_os_string().into_string().unwrap());
        let id = &name[..name.len()-11];
        let groups = user.get_string("groups");
        let mut da = DataArray::new();
        for group in groups.split(",") { da.push_str(group); }
        user.put_array("groups", da);
        users.put_object(id, user);
      }
    }
    
    if b { system.put_object("users", users.duplicate()); }
  }
}

pub fn load_config() -> DataObject {
  println!("Loading appserver configuration");
  let mut config;
  if Path::new("config.properties").exists() {
    config = read_properties("config.properties".to_string());
  }
  else { config = DataObject::new(); }
  
  if !config.has("security") { config.put_bool("security", true); }
  else { 
    let b = config.get_string("security") == "on";
    config.put_bool("security", b); 
    if !b { println!("Warning! Security is OFF!"); }
  }
  
  if !config.has("socket_address") {
    if !config.has("http_address") { config.put_str("http_address", "127.0.0.1"); }
    if !config.has("http_port") { config.put_str("http_port", "5774"); }
    
    let ip = config.get_string("http_address");
    let port = config.get_string("http_port");

    let socket_address = ip+":"+&port;
    config.put_str("socket_address", &socket_address);
  }
  
  if config.has("sessiontimeoutmillis") { 
    let d = config.get_property("sessiontimeoutmillis");
    if !d.is_int() { 
      let session_timeout = d.string().parse::<i64>().unwrap(); 
      config.duplicate().put_i64("sessiontimeoutmillis", session_timeout);
    }
  }
  else { 
    let session_timeout = 900000; 
    config.duplicate().put_i64("sessiontimeoutmillis", session_timeout);
  }
  
  if !config.has("apps") {
    config.put_str("apps", "app,lib");
  }
  
  if !config.has("default_app") {
    config.put_str("default_app", "app");
  }
  
  if !config.has("machineid") {
    config.put_str("machineid", "MY_DEVICE");
  }
  
  let mut system = DataStore::globals().get_object("system");
  system.put_object("config", config.duplicate());
  config
}

pub fn init_globals() -> DataObject {
  let mut globals = DataStore::globals();
  
  let mut system;
  if globals.has("system") { system = globals.get_object("system"); }
  else {
    system = DataObject::new();
    globals.put_object("system", system.duplicate());
  }

  let config = load_config();
  load_users();
  
  system.put_object("timers", DataObject::new());
  system.put_object("events", DataObject::new());
    
  let s = config.get_string("apps");
  let s = s.trim().to_string();
  let sa = s.split(",");
  
  let mut apps = DataObject::new();
  let default_app;
  if config.has("default_app") { default_app = config.get_string("default_app"); }
  else { default_app = sa.to_owned().nth(0).unwrap().to_string(); }
  
  let libraries = DataObject::new();
  system.put_object("libraries", libraries.duplicate());
  
  for i in sa {
    let mut o = DataObject::new();
    o.put_str("id", i);
    let path_base = "runtime/".to_string()+i+"/";
    let path = path_base.to_owned()+"botd.properties";
    let p = read_properties(path);
    o.put_object("runtime", p);
    let path = path_base+"app.properties";
    let p = read_properties(path);
    o.put_object("app", p.duplicate());
    apps.put_object(i, o);
    
    let s = p.get_string("libraries");
    let sa2 = s.split(",");
    for j in sa2 {
      if !libraries.has(j) { load_library(j); }
    }
  }
  
  system.put_str("default_app", &default_app);
  system.put_object("apps", apps);
  system.put_object("sessions", DataObject::new());
  system.put_bool("running", true);
  
  let store = DataStore::new();

  // Init Timers and Events
  for lib in libraries.duplicate().keys() {
    let controls = store.get_data(&lib, "controls").get_object("data").get_array("list");
    for ctldata in controls.objects() {
      let ctldata = ctldata.object();
      let ctlid = ctldata.get_string("id");
      let ctlname = ctldata.get_string("name");
      let ctl = store.get_data(&lib, &ctlid).get_object("data");
      if ctl.has("timer") {
        let ctimers = ctl.get_array("timer");
        for timer in ctimers.objects() {
          let timer = timer.object();
          let tname = timer.get_string("name");
          let tid = timer.get_string("id");
          let mut tdata = store.get_data(&lib, &tid).get_object("data");
          tdata.put_str("ctlname", &ctlname);
          tdata.put_str("name", &tname);
          if !tdata.has("start") { println!("Timer {}:{}:{} is not properly configured.", lib, ctlname, tname); }
          else { add_timer(&tid, tdata); }
        }
      }
      if ctl.has("event") {
        let cevents = ctl.get_array("event");
        for event in cevents.objects() {
          let event = event.object();
          let eid = event.get_string("id");
          let edata = store.get_data(&lib, &eid).get_object("data");
          if !edata.has("bot") { println!("Event listener {}:{}:{} is not properly configured.", lib, ctlname, event.get_string("name")); }
          else {
            let app = edata.get_string("bot");
            let ename = edata.get_string("event");
            let cmddb = edata.get_string("cmddb");
            let cmdid = edata.get_string("cmd");
            add_event_listener(&eid, &app, &ename, &cmddb, &cmdid);
          }
        }
      }
    }
  }
  
  let p = store.root;
  for file in fs::read_dir(&p).unwrap() {
    let path = file.unwrap().path();
    if path.is_dir() {
      let name:String = path.file_name().unwrap().to_str().unwrap().to_string();
      if !libraries.has(&name) { load_library(&name); }
    }  
  }
  
  system
}

fn to_millis(i:i64, s:String) -> i64 {
  if s == "milliseconds" { return i; }
  let i = i * 1000;
  if s == "seconds" { return i; }
  let i = i * 60;
  if s == "minutes" { return i; }
  let i = i * 60;
  if s == "hours" { return i; }
  let i = i * 24;
  if s != "days" { panic!("Unknown time unit for timer ({})", &s); }
  
  i
}

pub fn format_result(command:Command, o:DataObject) -> DataObject {
  let mut d;
                
  if command.return_type == "FLAT" { 
    if command.lang == "flow" && o.duplicate().keys().len() > 1 {
      d = o; 
    }
    else {
      if o.has("a") { d = o.get_object("a"); }
      else if o.has("data") { d = o.get_object("data"); }
      else { d = o.objects()[0].1.object(); }
    }
  }
  else {
    d = DataObject::new();
    let oo;
    if o.has("a") { oo = o.get_property("a"); }
    else if o.has("data") { oo = o.get_property("data"); }
    else if o.has("msg") { oo = o.get_property("msg"); }
    else { oo = o.objects()[0].1.clone(); }
    if command.return_type == "String" {
      d.set_property("msg", oo.clone());
    }
    else {
      d.set_property("data", oo.clone());
    }
  }
  
  if !d.has("status") { d.put_str("status", "ok"); }
  
  d
}

fn lookup_command_id(system: DataObject, app:String, cmd: String) -> (bool, String, String) {
  let mut b = false;
  let mut ctldb = "".to_string();
  let mut id = "".to_string();
  let apps = system.get_object("apps");
  if apps.has(&app) {
    let appdata = apps.get_object(&app).get_object("app");
    ctldb = appdata.get_string("ctldb");
    let ctlid = appdata.get_string("ctlid");
    let store = DataStore::new();
    let ctllist = store.get_data(&ctldb, &ctlid).get_object("data");
    if ctllist.has("cmd") {
      let ctllist = ctllist.get_array("cmd");
      for ctl in ctllist.objects() {
        let ctl = ctl.object();
        let name = ctl.get_string("name");
        if name == cmd {
          b = true;
          id = ctl.get_string("id");
          break;
        }
      }
      }
  }
  (b, ctldb, id)
}

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

