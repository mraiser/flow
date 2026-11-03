// Live ndata handles (src/jsndata.rs): a command's JSONObject / JSONArray
// arguments, and every object or array read from one, are handles on the
// same ndata that Rust and Python commands see - not copies. A Proxy makes
// each handle behave like a plain object or array: property access, `in`,
// `delete`, Object.keys/entries, spread, for..of, JSON.stringify, and for
// arrays Array.isArray, length, indexing and the Array methods.
//
// Writing a value: a handle is stored as itself (the same ndata); a plain
// object or array becomes a new ndata object or array (as a dict does in
// Python); an object with toJSON (Date, URL) is stored as what toJSON
// returns; Uint8Array / ArrayBuffer / typed arrays become bytes; undefined
// removes an object's key (and is null in an array, as in JSON); NaN and the
// infinities are null. A function, symbol, BigInt, Map, Set or Promise is a
// TypeError that names it, as is a cyclic structure.
//
// Reading: strings, numbers, booleans and null as themselves; objects and
// arrays as handles (a new handle each read, so compare contents, not
// identity); bytes as a Uint8Array copy.
//
// Installed after the web APIs and before the prelude, which takes its
// internal hooks from globalThis.__flowlang_ndata and removes them.
(() => {
  "use strict";
  const ops = Deno.core.ops;
  const WRAPPER = new WeakMap();   // proxy -> Nd (the cppgc handle)

  const KIND_MISSING = 0, KIND_NULL = 1, KIND_BOOLEAN = 2, KIND_INTEGER = 3, KIND_FLOAT = 4,
    KIND_STRING = 5, KIND_OBJECT = 6, KIND_ARRAY = 7, KIND_BYTES = 8;

  const isIndex = (k) => typeof k === "string" && /^(0|[1-9][0-9]*)$/.test(k) && Number(k) < 4294967295;

  const read = (w, key) => {
    switch (ops.op_nd_kind(w, key)) {
      case KIND_MISSING: return undefined;
      case KIND_NULL: return null;
      case KIND_BOOLEAN: return ops.op_nd_get_bool(w, key);
      case KIND_INTEGER:
      case KIND_FLOAT: return ops.op_nd_get_num(w, key);
      case KIND_STRING: return ops.op_nd_get_str(w, key);
      case KIND_OBJECT:
      case KIND_ARRAY: return handle(ops.op_nd_get_nd(w, key));
      case KIND_BYTES: return new Uint8Array(ops.op_nd_get_bytes(w, key));
    }
    return undefined;
  };

  const describe = (v) => {
    if (typeof v === "function") return "a function";
    if (typeof v === "symbol") return "a symbol";
    if (typeof v === "bigint") return "a BigInt (convert it: String(x) or Number(x))";
    if (v instanceof Map) return "a Map (convert it: Object.fromEntries(map))";
    if (v instanceof Set) return "a Set (convert it: [...set])";
    if (v instanceof Promise) return "a Promise (await it first)";
    if (v instanceof WeakMap || v instanceof WeakSet) return "a weak collection";
    return null;
  };

  const bytesOf = (v) => {
    if (v instanceof Uint8Array) return v;
    if (ArrayBuffer.isView(v)) return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
    if (v instanceof ArrayBuffer) return new Uint8Array(v);
    return null;
  };

  // Stores v at key in the container w; `seen` guards against cycles in
  // plain structures.
  const write = (w, key, v, seen) => {
    if (v === null) return ops.op_nd_set_null(w, key);
    switch (typeof v) {
      case "undefined":
        // JSON's rule: an object drops the key, an array holds null.
        return ops.op_nd_is_array(w) ? ops.op_nd_set_null(w, key) : ops.op_nd_delete(w, key);
      case "string": return ops.op_nd_set_str(w, key, v);
      case "number": return ops.op_nd_set_num(w, key, v);
      case "boolean": return ops.op_nd_set_bool(w, key, v);
    }
    const wrapper = WRAPPER.get(v);
    if (wrapper) return ops.op_nd_set_nd(w, key, wrapper);
    const why = describe(v);
    if (why) throw new TypeError(`could not be converted: ${why}`);
    const bytes = bytesOf(v);
    if (bytes) return ops.op_nd_set_bytes(w, key, bytes);
    if (typeof v.toJSON === "function") return write(w, key, v.toJSON(key), seen);
    if (seen.has(v)) throw new TypeError("could not be converted: a cyclic structure");
    seen.add(v);
    try {
      const child = ops.op_nd_new(Array.isArray(v));
      if (Array.isArray(v)) {
        for (let i = 0; i < v.length; i++) write(child, String(i), v[i] === undefined ? null : v[i], seen);
      } else {
        for (const k of Object.keys(v)) {
          if (v[k] !== undefined) write(child, k, v[k], seen);
        }
      }
      return ops.op_nd_set_nd(w, key, child);
    } finally {
      seen.delete(v);
    }
  };

  class NDataObject {}
  Object.defineProperty(NDataObject.prototype, Symbol.toStringTag, { value: "NDataObject" });

  const objectHandler = (w) => ({
    get(_t, k) {
      if (typeof k !== "string") return k === Symbol.toStringTag ? "NDataObject" : undefined;
      return read(w, k);
    },
    set(_t, k, v) {
      if (typeof k !== "string") return false;
      write(w, k, v, new Set());
      return true;
    },
    has(_t, k) { return typeof k === "string" && ops.op_nd_kind(w, k) !== KIND_MISSING; },
    deleteProperty(_t, k) {
      if (typeof k === "string") ops.op_nd_delete(w, k);
      return true;
    },
    ownKeys() { return ops.op_nd_keys(w); },
    getOwnPropertyDescriptor(_t, k) {
      if (typeof k !== "string" || ops.op_nd_kind(w, k) === KIND_MISSING) return undefined;
      return { value: read(w, k), writable: true, enumerable: true, configurable: true };
    },
    defineProperty(_t, k, d) {
      if (typeof k !== "string" || !("value" in d)) return false;
      write(w, k, d.value, new Set());
      return true;
    },
    getPrototypeOf() { return NDataObject.prototype; },
    setPrototypeOf() { return false; },
  });

  const arrayHandler = (w) => ({
    get(_t, k, receiver) {
      if (k === "length") return ops.op_nd_len(w);
      if (isIndex(k)) return read(w, k);
      return Reflect.get(Array.prototype, k, receiver);
    },
    set(_t, k, v) {
      if (k === "length") {
        const n = Number(v);
        if (!Number.isInteger(n) || n < 0) throw new RangeError("Invalid array length");
        ops.op_nd_set_len(w, n);
        return true;
      }
      if (!isIndex(k)) return false;
      write(w, k, v === undefined ? null : v, new Set());
      return true;
    },
    has(_t, k) {
      if (k === "length") return true;
      if (isIndex(k)) return Number(k) < ops.op_nd_len(w);
      return k in Array.prototype;
    },
    deleteProperty(_t, k) {
      if (isIndex(k)) ops.op_nd_delete(w, k);
      return true;
    },
    ownKeys() { return [...ops.op_nd_keys(w), "length"]; },
    getOwnPropertyDescriptor(_t, k) {
      if (k === "length") return { value: ops.op_nd_len(w), writable: true, enumerable: false, configurable: false };
      if (!isIndex(k) || Number(k) >= ops.op_nd_len(w)) return undefined;
      return { value: read(w, k), writable: true, enumerable: true, configurable: true };
    },
    defineProperty(t, k, d) {
      if (!("value" in d)) return false;
      return this.set(t, k, d.value);
    },
    setPrototypeOf() { return false; },
  });

  // A handle for the cppgc wrapper w: an object or array Proxy.
  const handle = (w) => {
    const p = ops.op_nd_is_array(w)
      ? new Proxy([], arrayHandler(w))
      : new Proxy(Object.create(null), objectHandler(w));
    WRAPPER.set(p, w);
    return p;
  };

  // A deep copy as plain JavaScript values (handles become objects/arrays).
  const toJS = (v) => {
    if (!WRAPPER.has(v)) return v;
    if (Array.isArray(v)) return Array.from(v, toJS);
    const o = {};
    for (const k of Object.keys(v)) o[k] = toJS(v[k]);
    return o;
  };

  Object.defineProperty(globalThis, "ndata", {
    value: Object.freeze({
      /** True for a handle on ndata (an object or array argument, or one read from it). */
      isHandle: (v) => WRAPPER.has(v),
      /** A deep copy of a handle as plain objects and arrays. */
      toJS,
      /** A new, empty ndata object or array, as a handle. */
      object: () => handle(ops.op_nd_new(false)),
      array: () => handle(ops.op_nd_new(true)),
      NDataObject,
    }),
    writable: true, configurable: true, enumerable: false,
  });

  // For the prelude only.
  Object.defineProperty(globalThis, "__flowlang_ndata", {
    value: {
      // A call's arguments, in declared order (containers as handles).
      args(callId) {
        const w = ops.op_flowlang_call_args(callId);
        const n = ops.op_nd_len(w);
        const out = [];
        for (let i = 0; i < n; i++) out.push(read(w, String(i)));
        return out;
      },
      // Writes a call's result; `what` names it in a conversion error.
      result(callId, v, what) {
        const w = ops.op_flowlang_call_out(callId);
        try { write(w, "0", v === undefined ? null : v, new Set()); }
        catch (e) { if (e instanceof TypeError) throw new TypeError(`${what} ${e.message}`); throw e; }
      },
      // A new array/object container holding values (for flowlang.call).
      arrayOf(values, what) {
        const w = ops.op_nd_new(true);
        values.forEach((v, i) => {
          try { write(w, String(i), v === undefined ? null : v, new Set()); }
          catch (e) { if (e instanceof TypeError) throw new TypeError(`${what(i)} ${e.message}`); throw e; }
        });
        return w;
      },
      objectOf(values, what) {
        const w = ops.op_nd_new(false);
        for (const k of Object.keys(values || {})) {
          try { write(w, k, values[k], new Set()); }
          catch (e) { if (e instanceof TypeError) throw new TypeError(`${what(k)} ${e.message}`); throw e; }
        }
        return w;
      },
      read,
      newArray: () => ops.op_nd_new(true),
    },
    configurable: true, enumerable: false, writable: false,
  });
})();
