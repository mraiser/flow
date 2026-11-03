//! JavaScript commands: the web platform APIs - fetch against a local HTTP
//! server, URL/URLSearchParams, text encoding, base64, crypto,
//! structuredClone, performance, AbortController.
#![cfg(feature = "javascript_runtime")]

use flowlang::jscmd::{run, JsCall};
use ndata::dataobject::DataObject;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Once, OnceLock};

static INIT: Once = Once::new();
static SEQ: AtomicUsize = AtomicUsize::new(0);

fn init() {
  INIT.call_once(|| { flowlang::init("data"); });
}

fn ret(body: &str) -> Result<DataObject, String> {
  init();
  run(&JsCall {
    label: format!("jsweb{}:ctl:cmd{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)),
    params: vec![],
    body: body.to_string(),
    returntype: "Any".to_string(),
    args: DataObject::new(),
    timeout: None,
  })
}

/// The command's value, parsed (ndata objects do not keep key order).
fn val(body: &str) -> serde_json::Value {
  let env = ret(body).unwrap_or_else(|e| panic!("{}\n--- body:\n{}", e, body));
  let mut o = DataObject::new();
  o.set_property("v", env.get_property("a"));
  serde_json::from_str::<serde_json::Value>(&o.to_string()).unwrap()["v"].clone()
}

fn json(s: &str) -> serde_json::Value { serde_json::from_str(s).unwrap() }

/// A small HTTP/1.1 server for the fetch tests, started once.
fn server() -> String {
  static ADDR: OnceLock<String> = OnceLock::new();
  ADDR.get_or_init(|| {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
      for stream in listener.incoming().flatten() {
        std::thread::spawn(move || {
          let mut reader = BufReader::new(stream.try_clone().unwrap());
          let mut line = String::new();
          if reader.read_line(&mut line).is_err() { return; }
          let mut parts = line.split_whitespace();
          let method = parts.next().unwrap_or("").to_string();
          let path = parts.next().unwrap_or("").to_string();
          let mut headers = Vec::new();
          let mut length = 0usize;
          loop {
            let mut h = String::new();
            if reader.read_line(&mut h).is_err() || h == "\r\n" || h.is_empty() { break; }
            let (k, v) = h.trim_end().split_once(": ").unwrap_or((h.trim_end(), ""));
            if k.eq_ignore_ascii_case("content-length") { length = v.parse().unwrap_or(0); }
            headers.push((k.to_lowercase(), v.to_string()));
          }
          let mut body = vec![0u8; length];
          let _ = reader.read_exact(&mut body);
          let header = |n: &str| headers.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).unwrap_or_default();
          let (status, extra, content): (&str, String, Vec<u8>) = match path.as_str() {
            "/hello" => ("200 OK", "Content-Type: text/plain\r\nX-Test: one\r\nSet-Cookie: a=1\r\nSet-Cookie: b=2\r\n".into(), b"hi there".to_vec()),
            "/json" => ("200 OK", "Content-Type: application/json\r\n".into(), br#"{"n":1,"list":[true,null]}"#.to_vec()),
            "/echo" => {
              let echo = serde_json::json!({
                "method": method,
                "body": String::from_utf8_lossy(&body),
                "type": header("content-type"),
                "custom": header("x-custom"),
              });
              ("200 OK", "Content-Type: application/json\r\n".into(), echo.to_string().into_bytes())
            }
            "/bytes" => ("200 OK", "Content-Type: application/octet-stream\r\n".into(), vec![0, 1, 2, 254, 255]),
            "/redirect" => ("302 Found", "Location: /hello\r\n".into(), Vec::new()),
            "/missing" => ("404 Not Found", "".into(), b"nope".to_vec()),
            "/slow" => { std::thread::sleep(std::time::Duration::from_millis(1500)); ("200 OK", "".into(), b"late".to_vec()) }
            _ => ("500 Internal Server Error", "".into(), Vec::new()),
          };
          let mut out = stream;
          let _ = write!(out, "HTTP/1.1 {}\r\n{}Content-Length: {}\r\nConnection: close\r\n\r\n", status, extra, content.len());
          let _ = out.write_all(&content);
        });
      }
    });
    addr
  }).clone()
}

// ---- fetch --------------------------------------------------------------------------------

#[test]
fn fetch_gets_text_status_and_headers() {
  let s = server();
  let v = val(&format!(r#"
    const r = await fetch("{s}/hello");
    return {{ ok: r.ok, status: r.status, statusText: r.statusText, type: r.headers.get("Content-Type"),
             test: r.headers.get("x-test"), cookies: r.headers.getSetCookie(), text: await r.text(),
             used: r.bodyUsed, url: r.url, redirected: r.redirected }};
  "#));
  assert_eq!(v, json(&format!(r#"{{"ok":true,"status":200,"statusText":"OK","type":"text/plain","test":"one",
    "cookies":["a=1","b=2"],"text":"hi there","used":true,"url":"{}/hello","redirected":false}}"#, s)));
}

#[test]
fn fetch_posts_json_and_reads_json() {
  let s = server();
  let v = val(&format!(r#"
    const r = await fetch("{s}/echo", {{
      method: "post", headers: {{ "Content-Type": "application/json", "X-Custom": "yes" }},
      body: JSON.stringify({{ a: 1 }}),
    }});
    return await r.json();
  "#));
  assert_eq!(v, json(r#"{"method":"POST","body":"{\"a\":1}","type":"application/json","custom":"yes"}"#));
  // A string body defaults to text/plain; URLSearchParams to form encoding.
  let v = val(&format!(r#"
    const a = await (await fetch("{s}/echo", {{ method: "PUT", body: "plain" }})).json();
    const b = await (await fetch("{s}/echo", {{ method: "POST", body: new URLSearchParams({{ q: "a b", n: "1&2" }}) }})).json();
    return [a.type, b.type, b.body];
  "#));
  assert_eq!(v, json(r#"["text/plain;charset=UTF-8","application/x-www-form-urlencoded;charset=UTF-8","q=a+b&n=1%262"]"#));
}

#[test]
fn fetch_reads_bytes_and_follows_redirects() {
  let s = server();
  let v = val(&format!(r#"
    const b = await (await fetch("{s}/bytes")).bytes();
    const r = await fetch("{s}/redirect");
    const m = await fetch("{s}/redirect", {{ redirect: "manual" }});
    let e = null;
    try {{ await fetch("{s}/redirect", {{ redirect: "error" }}); }} catch (x) {{ e = x.name; }}
    return {{ bytes: [...b], status: r.status, text: await r.text(), redirected: r.redirected,
             manual: m.status, location: m.headers.get("location"), error: e }};
  "#));
  assert_eq!(v, json(r#"{"bytes":[0,1,2,254,255],"status":200,"text":"hi there","redirected":true,"manual":302,"location":"/hello","error":"TypeError"}"#));
}

#[test]
fn an_http_error_status_is_a_response_not_a_rejection() {
  let s = server();
  let v = val(&format!(r#"const r = await fetch("{s}/missing"); return [r.ok, r.status, await r.text()];"#));
  assert_eq!(v, json(r#"[false,404,"nope"]"#));
}

#[test]
fn a_network_failure_rejects_with_a_type_error() {
  let v = val(r#"try { await fetch("http://127.0.0.1:1/"); return "fetched"; } catch (e) { return [e.name, e.message.startsWith("fetch failed")]; }"#);
  assert_eq!(v, json(r#"["TypeError",true]"#));
  let v = val(r#"try { await fetch("not a url"); } catch (e) { return e.name; }"#);
  assert_eq!(v, json(r#""TypeError""#));
}

#[test]
fn a_fetch_can_be_aborted() {
  let s = server();
  let started = std::time::Instant::now();
  let v = val(&format!(r#"
    const c = new AbortController();
    setTimeout(() => c.abort(), 50);
    try {{ await fetch("{s}/slow", {{ signal: c.signal }}); return "fetched"; }}
    catch (e) {{ return [e.name, c.signal.aborted]; }}
  "#));
  assert_eq!(v, json(r#"["AbortError",true]"#));
  assert!(started.elapsed() < std::time::Duration::from_millis(1200), "{:?}", started.elapsed());
  let v = val(&format!(r#"try {{ await fetch("{s}/slow", {{ signal: AbortSignal.timeout(50) }}); }} catch (e) {{ return e.name; }}"#));
  assert_eq!(v, json(r#""TimeoutError""#));
}

#[test]
fn response_and_headers_behave_like_the_standard() {
  let v = val(r#"
    const h = new Headers([["B", "2"], ["a", "1"]]);
    h.append("a", "3");
    const r = Response.json({ x: 1 }, { status: 201 });
    let used = null;
    const t = new Response("once");
    await t.text();
    try { await t.text(); } catch (e) { used = e.name; }
    return { entries: [...h], a: h.get("A"), has: h.has("b"), status: r.status, type: r.headers.get("content-type"),
             body: await r.json(), used, clone: await new Response("c").clone().text() };
  "#);
  assert_eq!(v, json(r#"{"entries":[["a","1, 3"],["b","2"]],"a":"1, 3","has":true,"status":201,"type":"application/json","body":{"x":1},"used":"TypeError","clone":"c"}"#));
}

// ---- URL ----------------------------------------------------------------------------------

#[test]
fn urls_parse_resolve_and_update() {
  let v = val(r##"
    const u = new URL("../b/c?x=1&y=two#frag", "https://user:pw@example.com:8443/a/z/index.html");
    const before = { href: u.href, origin: u.origin, protocol: u.protocol, host: u.host, hostname: u.hostname,
                     port: u.port, pathname: u.pathname, search: u.search, hash: u.hash, username: u.username,
                     y: u.searchParams.get("y") };
    u.searchParams.append("z", "a b");
    u.pathname = "/new path";
    u.port = "443";
    return { before, after: u.href, json: JSON.stringify({ u }), can: [URL.canParse("nope"), URL.canParse("data:,x")] };
  "##);
  assert_eq!(v, json(r##"{"before":{"href":"https://user:pw@example.com:8443/a/b/c?x=1&y=two#frag","origin":"https://example.com:8443",
    "protocol":"https:","host":"example.com:8443","hostname":"example.com","port":"8443","pathname":"/a/b/c","search":"?x=1&y=two",
    "hash":"#frag","username":"user","y":"two"},
    "after":"https://user:pw@example.com/new%20path?x=1&y=two&z=a+b#frag","json":"{\"u\":\"https://user:pw@example.com/new%20path?x=1&y=two&z=a+b#frag\"}","can":[false,true]}"##));
  let v = val(r#"try { new URL("/relative"); } catch (e) { return e.name; }"#);
  assert_eq!(v, json(r#""TypeError""#));
}

#[test]
fn search_params_encode_like_forms() {
  let v = val(r#"
    const p = new URLSearchParams("?a=1&b=x+y&a=%C3%A9&c");
    p.set("b", "1&2=3");
    p.delete("c");
    p.sort();
    return { all: p.getAll("a"), str: p.toString(), size: p.size, from: new URLSearchParams({ k: "ü v" }).toString() };
  "#);
  assert_eq!(v, json(r#"{"all":["1","é"],"str":"a=1&a=%C3%A9&b=1%262%3D3","size":3,"from":"k=%C3%BC+v"}"#));
}

// ---- text, base64, crypto, cloning, time --------------------------------------------------

#[test]
fn text_encoding_round_trips_utf8() {
  let v = val(r#"
    const bytes = new TextEncoder().encode("héllo €");
    const into = new Uint8Array(4);
    const r = new TextEncoder().encodeInto("é€", into);
    let fatal = null;
    try { new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array([0xff])); } catch (e) { fatal = e.name; }
    return { bytes: [...bytes], back: new TextDecoder().decode(bytes), into: [r.read, r.written],
             lossy: new TextDecoder().decode(new Uint8Array([0x61, 0xff])), fatal };
  "#);
  assert_eq!(v, json(r#"{"bytes":[104,195,169,108,108,111,32,226,130,172],"back":"héllo €","into":[1,2],"lossy":"a�","fatal":"TypeError"}"#));
}

#[test]
fn base64_matches_the_standard() {
  let v = val(r#"
    let bad = null;
    try { atob("***"); } catch (e) { bad = e.name; }
    return [btoa("hello"), btoa("a"), atob("aGVsbG8="), atob(" YQ = = "), bad];
  "#);
  assert_eq!(v, json(r#"["aGVsbG8=","YQ==","hello","a","InvalidCharacterError"]"#));
}

#[test]
fn crypto_clone_and_performance() {
  let v = val(r#"
    const a = crypto.getRandomValues(new Uint8Array(32));
    const b = crypto.getRandomValues(new Uint32Array(4));
    const id = crypto.randomUUID();
    const d = new Date(0);
    const c = structuredClone({ d, m: new Map([[1, 2]]), nested: [1, { x: 2 }] });
    const t0 = performance.now();
    await new Promise((r) => setTimeout(r, 20));
    return { varied: new Set(a).size > 8, words: b.length, uuid: /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(id),
             date: c.d instanceof Date && c.d !== d, map: c.m.get(1), nested: c.nested[1].x, waited: performance.now() - t0 >= 15 };
  "#);
  assert_eq!(v, json(r#"{"varied":true,"words":4,"uuid":true,"date":true,"map":2,"nested":2,"waited":true}"#));
}

#[test]
fn event_targets_and_abort_signals() {
  let v = val(r#"
    const seen = [];
    const t = new EventTarget();
    const f = (e) => seen.push(e.type);
    t.addEventListener("ping", f);
    t.addEventListener("once", f, { once: true });
    t.dispatchEvent(new Event("ping")); t.dispatchEvent(new Event("once")); t.dispatchEvent(new Event("once"));
    t.removeEventListener("ping", f); t.dispatchEvent(new Event("ping"));
    const c = new AbortController();
    c.signal.onabort = () => seen.push("aborted");
    c.abort("why");
    let thrown = null;
    try { c.signal.throwIfAborted(); } catch (e) { thrown = e; }
    return { seen, reason: c.signal.reason, thrown, pre: AbortSignal.abort().aborted };
  "#);
  assert_eq!(v, json(r#"{"seen":["ping","once","aborted"],"reason":"why","thrown":"why","pre":true}"#));
}
