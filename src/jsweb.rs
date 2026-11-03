//! Web platform APIs for JavaScript commands: the ops behind `jsweb.js`
//! (fetch, URL, crypto, performance). `jsweb.js` builds the standard
//! classes on these; the engine installs it before its prelude.
//!
//! `fetch` runs the request with `ureq` on tokio's blocking pool, so the
//! engine keeps serving other calls while a request is in flight. TLS
//! verification uses the operating system's trust store (and
//! `SSL_CERT_FILE`); `HTTPS_PROXY` / `HTTP_PROXY` / `ALL_PROXY` /
//! `NO_PROXY` are honoured. A body is read whole (up to 256 MiB).

use std::sync::OnceLock;
use std::time::Instant;

use deno_core::op2;
use deno_core::url::{quirks, Url};
use deno_core::{JsBuffer, ToJsBuffer};
use serde::{Deserialize, Serialize};

pub(crate) const WEB_JS: &str = include_str!("jsweb.js");

/// The most a fetched body may hold.
const MAX_BODY: u64 = 256 * 1024 * 1024;

static CLOCK: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since the first JavaScript engine started, with sub-ms
/// precision: `performance.now()`.
#[op2(fast)]
pub fn op_flowlang_now() -> f64 {
  CLOCK.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// Cryptographically secure random bytes (the OS source; flowlang::rand):
/// `crypto.getRandomValues`, `crypto.randomUUID`.
#[op2]
#[buffer]
pub fn op_flowlang_random(#[smi] len: u32) -> Vec<u8> {
  let mut v = vec![0u8; len.min(65536) as usize];
  crate::rand::fill_bytes(&mut v);
  v
}

/// A URL's components as the WHATWG URL standard defines them.
#[derive(Serialize)]
pub(crate) struct UrlParts {
  href: String,
  origin: String,
  protocol: String,
  username: String,
  password: String,
  host: String,
  hostname: String,
  port: String,
  pathname: String,
  search: String,
  hash: String,
}

fn parts(u: &Url) -> UrlParts {
  UrlParts {
    href: quirks::href(u).to_string(),
    origin: quirks::origin(u),
    protocol: quirks::protocol(u).to_string(),
    username: quirks::username(u).to_string(),
    password: quirks::password(u).to_string(),
    host: quirks::host(u).to_string(),
    hostname: quirks::hostname(u).to_string(),
    port: quirks::port(u).to_string(),
    pathname: quirks::pathname(u).to_string(),
    search: quirks::search(u).to_string(),
    hash: quirks::hash(u).to_string(),
  }
}

#[derive(Serialize)]
pub(crate) enum Answer<T> {
  #[serde(rename = "ok")]
  Ok(T),
  #[serde(rename = "err")]
  Err(String),
}

#[derive(Deserialize)]
pub(crate) struct UrlArgs {
  href: String,
  base: Option<String>,
}

/// `new URL(href, base)`: `{ok: parts}` or `{err: why}`.
#[op2]
#[serde]
pub(crate) fn op_flowlang_url_parse(#[serde] args: UrlArgs) -> Answer<UrlParts> {
  let UrlArgs { href, base } = args;
  let parsed = match base {
    None => Url::parse(&href),
    Some(b) => match Url::parse(&b) {
      Ok(base) => base.join(&href),
      Err(e) => return Answer::Err(format!("the base: {}", e)),
    },
  };
  match parsed {
    Ok(u) => Answer::Ok(parts(&u)),
    Err(e) => Answer::Err(e.to_string()),
  }
}

/// A URL setter (`url.pathname = ...`), with the standard's semantics:
/// `{ok: parts}`, or `{err}` when the standard ignores the new value.
#[op2]
#[serde]
pub(crate) fn op_flowlang_url_set(#[string] href: String, #[string] component: String, #[string] value: String) -> Answer<UrlParts> {
  let mut u = match Url::parse(&href) {
    Ok(u) => u,
    Err(e) => return Answer::Err(e.to_string()),
  };
  let ok = match component.as_str() {
    "href" => quirks::set_href(&mut u, &value).map_err(|_| ()),
    "protocol" => quirks::set_protocol(&mut u, &value),
    "username" => quirks::set_username(&mut u, &value),
    "password" => quirks::set_password(&mut u, &value),
    "host" => quirks::set_host(&mut u, &value),
    "hostname" => quirks::set_hostname(&mut u, &value),
    "port" => quirks::set_port(&mut u, &value),
    "pathname" => { quirks::set_pathname(&mut u, &value); Ok(()) }
    "search" => { quirks::set_search(&mut u, &value); Ok(()) }
    "hash" => { quirks::set_hash(&mut u, &value); Ok(()) }
    _ => Err(()),
  };
  match ok {
    Ok(()) => Answer::Ok(parts(&u)),
    Err(()) => Answer::Err(format!("{} cannot be set to {:?}", component, value)),
  }
}

#[derive(Deserialize)]
pub struct FetchRequest {
  url: String,
  method: String,
  headers: Vec<(String, String)>,
  body: Option<JsBuffer>,
  redirect: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchResponse {
  status: u16,
  status_text: String,
  headers: Vec<(String, String)>,
  body: ToJsBuffer,
  url: String,
  redirected: bool,
}

/// `fetch`: `{ok: response}` or `{err: why}` (jsweb.js throws it as a
/// TypeError, as browsers do for a network error).
#[op2]
#[serde]
pub(crate) async fn op_flowlang_fetch(#[serde] req: FetchRequest) -> Answer<FetchResponse> {
  let FetchRequest { url, method, headers, body, redirect } = req;
  let body = body.map(|b| b.to_vec());
  match tokio::task::spawn_blocking(move || blocking_fetch(&url, &method, &headers, body, &redirect)).await {
    Ok(Ok(r)) => Answer::Ok(r),
    Ok(Err(e)) => Answer::Err(e),
    Err(e) => Answer::Err(format!("the request thread failed: {}", e)),
  }
}

fn agent() -> &'static ureq::Agent {
  static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
  AGENT.get_or_init(|| {
    ureq::Agent::config_builder()
      .http_status_as_error(false)
      .allow_non_standard_methods(true)
      .save_redirect_history(true)
      .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
      .proxy(ureq::Proxy::try_from_env())
      .build()
      .into()
  })
}

fn blocking_fetch(url: &str, method: &str, headers: &[(String, String)], body: Option<Vec<u8>>, redirect: &str) -> Result<FetchResponse, String> {
  use ureq::ResponseExt;
  let mut builder = ureq::http::Request::builder().method(method).uri(url);
  for (k, v) in headers {
    builder = builder.header(k.as_str(), v.as_str());
  }
  // "follow": up to 20 redirects (the standard's limit), then an error;
  // "manual": the redirect response itself; "error": a redirect is an error.
  // (ureq never reports a redirect when it may follow none, so "error" is
  // checked here.)
  let (max, will_error) = if redirect == "follow" { (20, true) } else { (0, false) };
  let timeout = crate::jscmd::default_timeout();
  let response = match body {
    Some(bytes) => {
      let request = builder.body(bytes).map_err(|e| e.to_string())?;
      let request = agent().configure_request(request)
        .max_redirects(max).max_redirects_will_error(will_error).timeout_global(timeout).build();
      agent().run(request)
    }
    None => {
      let request = builder.body(()).map_err(|e| e.to_string())?;
      let request = agent().configure_request(request)
        .max_redirects(max).max_redirects_will_error(will_error).timeout_global(timeout).build();
      agent().run(request)
    }
  };
  let mut response = response.map_err(|e| e.to_string())?;
  let status = response.status();
  if redirect == "error" && status.is_redirection() && response.headers().contains_key("location") {
    return Err(format!("{} redirected (status {}) and the request's redirect mode is \"error\"", url, status.as_u16()));
  }
  let headers = response.headers().iter()
    .map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).to_string()))
    .collect();
  let final_url = response.get_uri().to_string();
  let redirected = response.get_redirect_history().map_or(false, |h| h.len() > 1);
  let bytes = response.body_mut().with_config().limit(MAX_BODY).read_to_vec().map_err(|e| e.to_string())?;
  Ok(FetchResponse {
    status: status.as_u16(),
    status_text: status.canonical_reason().unwrap_or("").to_string(),
    headers,
    body: ToJsBuffer::from(bytes),
    url: final_url,
    redirected,
  })
}
