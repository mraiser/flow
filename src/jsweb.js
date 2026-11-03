// Web platform APIs for JavaScript commands, on top of V8 and flowlang's ops
// (src/jsweb.rs): fetch, Headers, Request, Response, URL, URLSearchParams,
// TextEncoder, TextDecoder, atob, btoa, crypto.getRandomValues/randomUUID,
// structuredClone, performance.now, Event, EventTarget, AbortController,
// AbortSignal, DOMException. Installed once per isolate, before the prelude.
// Behaviour follows the web standards where it matters to command code; the
// differences are documented in docs/js-commands.md (no streaming bodies:
// a body is read whole; no Blob/FormData).
(() => {
  "use strict";
  const core = Deno.core;
  const ops = core.ops;
  const define = (name, value) =>
    Object.defineProperty(globalThis, name, { value, writable: true, configurable: true, enumerable: false });

  // ---- DOMException, Event, EventTarget ------------------------------------------------

  class DOMException extends Error {
    constructor(message = "", name = "Error") {
      super(message);
      Object.defineProperty(this, "name", { value: name, configurable: true, writable: true });
    }
  }

  class Event {
    #type;
    constructor(type) { this.#type = String(type); this.defaultPrevented = false; this.target = null; }
    get type() { return this.#type; }
    preventDefault() { this.defaultPrevented = true; }
  }

  class EventTarget {
    #listeners = new Map();
    addEventListener(type, listener, options) {
      if (typeof listener !== "function" && !(listener && typeof listener.handleEvent === "function")) return;
      const once = typeof options === "object" && options !== null && !!options.once;
      const list = this.#listeners.get(type) || [];
      if (!list.some((l) => l.listener === listener)) list.push({ listener, once });
      this.#listeners.set(type, list);
    }
    removeEventListener(type, listener) {
      const list = this.#listeners.get(type);
      if (list) this.#listeners.set(type, list.filter((l) => l.listener !== listener));
    }
    dispatchEvent(event) {
      try { event.target = this; } catch (_) {}
      const handler = this["on" + event.type];
      if (typeof handler === "function") handler.call(this, event);
      for (const l of [...(this.#listeners.get(event.type) || [])]) {
        if (l.once) this.removeEventListener(event.type, l.listener);
        if (typeof l.listener === "function") l.listener.call(this, event);
        else l.listener.handleEvent(event);
      }
      return !event.defaultPrevented;
    }
  }

  // ---- AbortController / AbortSignal ----------------------------------------------------

  const abortSignal = Symbol("abort");
  class AbortSignal extends EventTarget {
    #aborted = false;
    #reason = undefined;
    constructor(key) {
      super();
      if (key !== abortSignal) throw new TypeError("Illegal constructor");
      this.onabort = null;
    }
    get aborted() { return this.#aborted; }
    get reason() { return this.#reason; }
    throwIfAborted() { if (this.#aborted) throw this.#reason; }
    [abortSignal](reason) {
      if (this.#aborted) return;
      this.#aborted = true;
      this.#reason = reason === undefined ? new DOMException("This operation was aborted", "AbortError") : reason;
      this.dispatchEvent(new Event("abort"));
    }
    static abort(reason) {
      const s = new AbortSignal(abortSignal);
      s[abortSignal](reason);
      return s;
    }
    static timeout(ms) {
      const s = new AbortSignal(abortSignal);
      setTimeout(() => s[abortSignal](new DOMException("The operation timed out.", "TimeoutError")), ms);
      return s;
    }
    static any(signals) {
      const s = new AbortSignal(abortSignal);
      for (const other of signals) {
        if (other.aborted) { s[abortSignal](other.reason); break; }
        other.addEventListener("abort", () => s[abortSignal](other.reason), { once: true });
      }
      return s;
    }
  }
  class AbortController {
    #signal = new AbortSignal(abortSignal);
    get signal() { return this.#signal; }
    abort(reason) { this.#signal[abortSignal](reason); }
  }

  // ---- text, base64, crypto, structuredClone, performance -------------------------------

  const toBytes = (input) => {
    if (input instanceof Uint8Array) return input;
    if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    if (input instanceof ArrayBuffer || (typeof SharedArrayBuffer !== "undefined" && input instanceof SharedArrayBuffer)) return new Uint8Array(input);
    return null;
  };

  class TextEncoder {
    get encoding() { return "utf-8"; }
    encode(input = "") { return core.encode(String(input)); }
    encodeInto(input, dest) {
      const bytes = core.encode(String(input));
      // Whole code points only: never split a UTF-8 sequence at the end.
      let written = Math.min(bytes.length, dest.length);
      while (written > 0 && written < bytes.length && (bytes[written] & 0xc0) === 0x80) written--;
      dest.set(bytes.subarray(0, written));
      const read = core.decode(bytes.subarray(0, written)).length;
      return { read, written };
    }
  }

  class TextDecoder {
    #fatal; #ignoreBOM;
    constructor(label = "utf-8", options = {}) {
      const l = String(label).trim().toLowerCase();
      if (l !== "utf-8" && l !== "utf8" && l !== "unicode-1-1-utf-8") {
        throw new RangeError(`TextDecoder: only utf-8 is supported, not "${label}"`);
      }
      this.#fatal = !!options.fatal;
      this.#ignoreBOM = !!options.ignoreBOM;
    }
    get encoding() { return "utf-8"; }
    get fatal() { return this.#fatal; }
    get ignoreBOM() { return this.#ignoreBOM; }
    decode(input) {
      if (input === undefined) return "";
      let bytes = toBytes(input);
      if (!bytes) throw new TypeError("TextDecoder.decode: expected an ArrayBuffer or a view of one");
      if (!this.#ignoreBOM && bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) bytes = bytes.subarray(3);
      const text = core.decode(bytes);
      // Valid UTF-8 round-trips byte for byte; anything else was replaced.
      if (this.#fatal && text.includes("\ufffd") && !sameBytes(core.encode(text), bytes)) {
        throw new TypeError("The encoded data was not valid utf-8");
      }
      return text;
    }
  }
  const sameBytes = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

  const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const btoa = (data) => {
    const s = String(data);
    let out = "";
    for (let i = 0; i < s.length; i += 3) {
      const c = [s.charCodeAt(i), s.charCodeAt(i + 1), s.charCodeAt(i + 2)];
      for (const x of c) if (x > 255) throw new DOMException("The string to be encoded contains characters outside of the Latin1 range.", "InvalidCharacterError");
      const n = (c[0] << 16) | ((c[1] || 0) << 8) | (c[2] || 0);
      out += B64[(n >> 18) & 63] + B64[(n >> 12) & 63] + (i + 1 < s.length ? B64[(n >> 6) & 63] : "=") + (i + 2 < s.length ? B64[n & 63] : "=");
    }
    return out;
  };
  const atob = (data) => {
    let s = String(data).replace(/[\t\n\f\r ]/g, "");
    if (s.length % 4 === 0) s = s.replace(/==?$/, "");
    if (s.length % 4 === 1 || /[^A-Za-z0-9+/]/.test(s)) {
      throw new DOMException("The string to be decoded is not correctly encoded.", "InvalidCharacterError");
    }
    let out = "", bits = 0, acc = 0;
    for (const ch of s) {
      acc = (acc << 6) | B64.indexOf(ch);
      bits += 6;
      if (bits >= 8) { bits -= 8; out += String.fromCharCode((acc >> bits) & 255); }
    }
    return out;
  };

  const crypto = Object.freeze({
    getRandomValues(array) {
      if (!ArrayBuffer.isView(array) || array instanceof Float32Array || array instanceof Float64Array || array instanceof DataView) {
        throw new DOMException("The provided value is not an integer-typed array", "TypeMismatchError");
      }
      if (array.byteLength > 65536) throw new DOMException("The ArrayBufferView's byte length exceeds 65536", "QuotaExceededError");
      new Uint8Array(array.buffer, array.byteOffset, array.byteLength).set(ops.op_flowlang_random(array.byteLength));
      return array;
    },
    randomUUID() {
      const b = ops.op_flowlang_random(16);
      b[6] = (b[6] & 0x0f) | 0x40;
      b[8] = (b[8] & 0x3f) | 0x80;
      const h = [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
      return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
    },
  });

  const structuredClone = (value, options) => {
    if (options && options.transfer && options.transfer.length) throw new DOMException("structuredClone: transfer is not supported", "DataCloneError");
    try { return core.structuredClone(value); }
    catch (e) { throw new DOMException(String(e && e.message || e), "DataCloneError"); }
  };

  const timeOrigin = Date.now() - ops.op_flowlang_now();
  const performance = Object.freeze({
    now: () => ops.op_flowlang_now(),
    get timeOrigin() { return timeOrigin; },
  });

  // ---- URL, URLSearchParams --------------------------------------------------------------

  const formEncode = (s) => {
    let out = "";
    for (const b of core.encode(String(s))) {
      const c = String.fromCharCode(b);
      if (/[A-Za-z0-9*\-._]/.test(c)) out += c;
      else if (b === 0x20) out += "+";
      else out += "%" + b.toString(16).toUpperCase().padStart(2, "0");
    }
    return out;
  };
  const formDecode = (s) => {
    const bytes = [];
    const t = String(s).replace(/\+/g, " ");
    for (let i = 0; i < t.length; i++) {
      if (t[i] === "%" && /^[0-9A-Fa-f]{2}$/.test(t.slice(i + 1, i + 3))) { bytes.push(parseInt(t.slice(i + 1, i + 3), 16)); i += 2; }
      else for (const b of core.encode(t[i])) bytes.push(b);
    }
    return core.decode(new Uint8Array(bytes));
  };

  const searchOwner = Symbol("url");
  class URLSearchParams {
    #list = [];
    constructor(init = "") {
      this[searchOwner] = null;
      if (init instanceof URLSearchParams) this.#list = [...init.#list.map((p) => [...p])];
      else if (typeof init === "object" && init !== null) {
        if (typeof init[Symbol.iterator] === "function") {
          for (const pair of init) {
            const p = [...pair];
            if (p.length !== 2) throw new TypeError("URLSearchParams: each pair must have exactly two items");
            this.#list.push([String(p[0]), String(p[1])]);
          }
        } else for (const k of Object.keys(init)) this.#list.push([k, String(init[k])]);
      } else this.#parse(String(init));
    }
    #parse(s) {
      this.#list = [];
      for (const part of s.replace(/^\?/, "").split("&")) {
        if (!part) continue;
        const i = part.indexOf("=");
        this.#list.push(i < 0 ? [formDecode(part), ""] : [formDecode(part.slice(0, i)), formDecode(part.slice(i + 1))]);
      }
    }
    #changed() { if (this[searchOwner]) this[searchOwner]._setSearchFromParams(this.toString()); }
    _reset(s) { this.#parse(s); }
    get size() { return this.#list.length; }
    append(k, v) { this.#list.push([String(k), String(v)]); this.#changed(); }
    delete(k, v) {
      this.#list = this.#list.filter(([a, b]) => !(a === String(k) && (v === undefined || b === String(v))));
      this.#changed();
    }
    get(k) { const p = this.#list.find(([a]) => a === String(k)); return p ? p[1] : null; }
    getAll(k) { return this.#list.filter(([a]) => a === String(k)).map(([, b]) => b); }
    has(k, v) { return this.#list.some(([a, b]) => a === String(k) && (v === undefined || b === String(v))); }
    set(k, v) {
      k = String(k); v = String(v);
      const i = this.#list.findIndex(([a]) => a === k);
      if (i < 0) this.#list.push([k, v]);
      else { this.#list[i][1] = v; this.#list = this.#list.filter(([a], j) => a !== k || j === i); }
      this.#changed();
    }
    sort() { this.#list.sort((x, y) => (x[0] < y[0] ? -1 : x[0] > y[0] ? 1 : 0)); this.#changed(); }
    forEach(fn, thisArg) { for (const [k, v] of this.#list) fn.call(thisArg, v, k, this); }
    *entries() { for (const p of this.#list) yield [p[0], p[1]]; }
    *keys() { for (const p of this.#list) yield p[0]; }
    *values() { for (const p of this.#list) yield p[1]; }
    [Symbol.iterator]() { return this.entries(); }
    toString() { return this.#list.map(([k, v]) => formEncode(k) + "=" + formEncode(v)).join("&"); }
  }

  const parseURL = (href, base) => {
    const r = ops.op_flowlang_url_parse({ href: String(href), base: base === undefined ? null : String(base) });
    if (r.err) throw new TypeError(`Invalid URL: ${JSON.stringify(String(href))}${base === undefined ? "" : ` with base ${JSON.stringify(String(base))}`} (${r.err})`);
    return r.ok;
  };
  class URL {
    #parts; #params;
    constructor(url, base) {
      this.#parts = parseURL(url instanceof URL ? url.href : url, base instanceof URL ? base.href : base);
      this.#params = new URLSearchParams(this.#parts.search);
      this.#params[searchOwner] = this;
    }
    static canParse(url, base) { try { new URL(url, base); return true; } catch (_) { return false; } }
    static parse(url, base) { try { return new URL(url, base); } catch (_) { return null; } }
    #set(component, value) {
      const r = ops.op_flowlang_url_set(this.#parts.href, component, String(value));
      if (r.err) { if (component === "href") throw new TypeError(`Invalid URL: ${JSON.stringify(String(value))}`); return; }
      this.#parts = r.ok;
      if (component === "href" || component === "search") this.#params._reset(this.#parts.search);
    }
    _setSearchFromParams(s) {
      const r = ops.op_flowlang_url_set(this.#parts.href, "search", s);
      if (!r.err) this.#parts = r.ok;
    }
    get href() { return this.#parts.href; } set href(v) { this.#set("href", v); }
    get origin() { return this.#parts.origin; }
    get protocol() { return this.#parts.protocol; } set protocol(v) { this.#set("protocol", v); }
    get username() { return this.#parts.username; } set username(v) { this.#set("username", v); }
    get password() { return this.#parts.password; } set password(v) { this.#set("password", v); }
    get host() { return this.#parts.host; } set host(v) { this.#set("host", v); }
    get hostname() { return this.#parts.hostname; } set hostname(v) { this.#set("hostname", v); }
    get port() { return this.#parts.port; } set port(v) { this.#set("port", v); }
    get pathname() { return this.#parts.pathname; } set pathname(v) { this.#set("pathname", v); }
    get search() { return this.#parts.search; } set search(v) { this.#set("search", v); }
    get hash() { return this.#parts.hash; } set hash(v) { this.#set("hash", v); }
    get searchParams() { return this.#params; }
    toString() { return this.href; }
    toJSON() { return this.href; }
  }

  // ---- Headers, Request, Response, fetch -------------------------------------------------

  const normalizeValue = (v) => String(v).replace(/^[\t\n\r ]+|[\t\n\r ]+$/g, "");
  const checkName = (n) => {
    if (!/^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(n)) throw new TypeError(`Invalid header name: ${JSON.stringify(n)}`);
    return n.toLowerCase();
  };
  class Headers {
    #list = []; // [lowercased name, value]
    constructor(init) {
      if (init === undefined || init === null) return;
      if (init instanceof Headers) { for (const [k, v] of init.#list) this.#list.push([k, v]); return; }
      if (typeof init[Symbol.iterator] === "function") {
        for (const pair of init) {
          const p = [...pair];
          if (p.length !== 2) throw new TypeError("Headers: each pair must have exactly two items");
          this.append(p[0], p[1]);
        }
      } else if (typeof init === "object") for (const k of Object.keys(init)) this.append(k, init[k]);
      else throw new TypeError("Headers: expected an object, an array of pairs or Headers");
    }
    append(name, value) { this.#list.push([checkName(String(name)), normalizeValue(value)]); }
    delete(name) { const n = checkName(String(name)); this.#list = this.#list.filter(([k]) => k !== n); }
    get(name) {
      const n = checkName(String(name));
      const vs = this.#list.filter(([k]) => k === n).map(([, v]) => v);
      return vs.length ? vs.join(", ") : null;
    }
    getSetCookie() { return this.#list.filter(([k]) => k === "set-cookie").map(([, v]) => v); }
    has(name) { const n = checkName(String(name)); return this.#list.some(([k]) => k === n); }
    set(name, value) {
      const n = checkName(String(name));
      const i = this.#list.findIndex(([k]) => k === n);
      const v = normalizeValue(value);
      if (i < 0) this.#list.push([n, v]);
      else { this.#list[i][1] = v; this.#list = this.#list.filter(([k], j) => k !== n || j === i); }
    }
    *entries() {
      const names = [...new Set(this.#list.map(([k]) => k))].sort();
      for (const n of names) {
        if (n === "set-cookie") for (const v of this.getSetCookie()) yield [n, v];
        else yield [n, this.get(n)];
      }
    }
    *keys() { for (const [k] of this.entries()) yield k; }
    *values() { for (const [, v] of this.entries()) yield v; }
    forEach(fn, thisArg) { for (const [k, v] of this.entries()) fn.call(thisArg, v, k, this); }
    [Symbol.iterator]() { return this.entries(); }
    _pairs() { return this.#list.map(([k, v]) => [k, v]); }
  }

  // A body as bytes plus the content type it implies.
  const extractBody = (body) => {
    if (body === undefined || body === null) return [null, null];
    if (typeof body === "string") return [core.encode(body), "text/plain;charset=UTF-8"];
    if (body instanceof URLSearchParams) return [core.encode(body.toString()), "application/x-www-form-urlencoded;charset=UTF-8"];
    const bytes = toBytes(body);
    if (bytes) return [new Uint8Array(bytes), null];
    throw new TypeError("a body must be a string, URLSearchParams, an ArrayBuffer or a typed array (JSON.stringify objects)");
  };

  class Body {
    #bytes; #used = false;
    constructor(bytes) { this.#bytes = bytes; }
    get bodyUsed() { return this.#used; }
    get body() { return null; }
    #take() {
      if (this.#used) return Promise.reject(new TypeError("Body has already been consumed"));
      this.#used = true;
      return Promise.resolve(this.#bytes || new Uint8Array(0));
    }
    _peek() { return this.#bytes; }
    async arrayBuffer() { const b = await this.#take(); return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength); }
    async bytes() { return new Uint8Array(await this.#take()); }
    async text() { return core.decode(await this.#take()); }
    async json() { return JSON.parse(await this.text()); }
  }

  const METHODS = ["DELETE", "GET", "HEAD", "OPTIONS", "POST", "PUT", "PATCH"];
  class Request extends Body {
    #url; #method; #headers; #signal; #redirect;
    constructor(input, init = {}) {
      const from = input instanceof Request ? input : null;
      const [bytes, type] = init.body !== undefined ? extractBody(init.body) : [from ? from._peek() : null, null];
      super(bytes);
      this.#url = from ? from.url : new URL(input).href;
      const m = String(init.method || (from ? from.method : "GET"));
      this.#method = METHODS.includes(m.toUpperCase()) ? m.toUpperCase() : m;
      if ((this.#method === "GET" || this.#method === "HEAD") && bytes) throw new TypeError("Request with GET/HEAD method cannot have body.");
      this.#headers = new Headers(init.headers !== undefined ? init.headers : from ? from.headers : undefined);
      if (type && !this.#headers.has("content-type")) this.#headers.set("content-type", type);
      this.#signal = init.signal !== undefined ? init.signal : from ? from.signal : null;
      this.#redirect = init.redirect || (from ? from.redirect : "follow");
      if (!["follow", "manual", "error"].includes(this.#redirect)) throw new TypeError(`redirect must be "follow", "manual" or "error"`);
    }
    get url() { return this.#url; }
    get method() { return this.#method; }
    get headers() { return this.#headers; }
    get signal() { return this.#signal; }
    get redirect() { return this.#redirect; }
    clone() { return new Request(this); }
  }

  const STATUS_TEXT = { 200: "OK", 201: "Created", 204: "No Content", 301: "Moved Permanently", 302: "Found", 304: "Not Modified", 400: "Bad Request", 401: "Unauthorized", 403: "Forbidden", 404: "Not Found", 500: "Internal Server Error" };
  const responseInternal = Symbol("response");
  class Response extends Body {
    #status; #statusText; #headers; #url = ""; #redirected = false; #type = "default";
    constructor(body = null, init = {}) {
      const [bytes, type] = extractBody(body);
      super(bytes);
      this.#status = init.status === undefined ? 200 : Number(init.status);
      if (!(this.#status >= 200 && this.#status <= 599)) throw new RangeError("status must be in the range 200-599");
      this.#statusText = init.statusText === undefined ? "" : String(init.statusText);
      this.#headers = new Headers(init.headers);
      if (type && !this.#headers.has("content-type")) this.#headers.set("content-type", type);
      if (init[responseInternal]) {
        this.#url = init[responseInternal].url;
        this.#redirected = init[responseInternal].redirected;
        this.#type = "basic";
      }
    }
    static json(data, init = {}) {
      const headers = new Headers(init.headers);
      if (!headers.has("content-type")) headers.set("content-type", "application/json");
      return new Response(JSON.stringify(data), { ...init, headers });
    }
    static error() { const r = new Response(null, { status: 200 }); r.#status = 0; r.#type = "error"; return r; }
    static redirect(url, status = 302) { return new Response(null, { status, headers: { location: new URL(url).href } }); }
    get status() { return this.#status; }
    get statusText() { return this.#statusText; }
    get ok() { return this.#status >= 200 && this.#status <= 299; }
    get headers() { return this.#headers; }
    get url() { return this.#url; }
    get redirected() { return this.#redirected; }
    get type() { return this.#type; }
    clone() {
      if (this.bodyUsed) throw new TypeError("Response.clone: Body has already been consumed");
      return new Response(this._peek(), { status: this.#status, statusText: this.#statusText, headers: this.#headers, [responseInternal]: { url: this.#url, redirected: this.#redirected } });
    }
  }

  const fetch = (input, init = {}) => {
    let request;
    try { request = new Request(input, init); } catch (e) { return Promise.reject(e); }
    const signal = request.signal;
    if (signal && signal.aborted) return Promise.reject(signal.reason);
    const pending = ops.op_flowlang_fetch({
      url: request.url,
      method: request.method,
      headers: request.headers._pairs(),
      body: request._peek(),
      redirect: request.redirect,
    }).then((r) => {
      if (r.err) throw new TypeError(`fetch failed: ${r.err}`);
      const res = r.ok;
      // A 1xx/204/304-style status cannot be given to the Response
      // constructor, which takes 200-599: build with 200 and set it after.
      const response = new Response(res.status === 204 || res.status === 304 || res.status < 200 ? null : res.body, {
        status: res.status >= 200 && res.status <= 599 ? res.status : 200,
        statusText: res.statusText,
        headers: res.headers,
        [responseInternal]: { url: res.url, redirected: res.redirected },
      });
      return response;
    });
    if (!signal) return pending;
    return new Promise((resolve, reject) => {
      const onAbort = () => reject(signal.reason);
      signal.addEventListener("abort", onAbort, { once: true });
      pending.then((v) => { signal.removeEventListener("abort", onAbort); resolve(v); },
        (e) => { signal.removeEventListener("abort", onAbort); reject(e); });
    });
  };

  for (const [name, value] of Object.entries({
    DOMException, Event, EventTarget, AbortController, AbortSignal,
    TextEncoder, TextDecoder, atob, btoa, crypto, structuredClone, performance,
    URL, URLSearchParams, Headers, Request, Response, fetch,
  })) define(name, value);
})();
