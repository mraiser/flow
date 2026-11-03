// The globals a JavaScript command sees in flowlang's engine (javascript_runtime),
// beyond the ECMAScript standard library. Written by flowb next to the generated
// command sources; check_js type-checks command bodies against it. Nothing else is
// defined: no require, process, Buffer or DOM.

interface Console {
  /** stdout (stderr under flowmcp), as lines labelled `[js lib:ctl:cmd]`. */
  log(...data: any[]): void;
  info(...data: any[]): void;
  debug(...data: any[]): void;
  /** stderr, as lines labelled `[js lib:ctl:cmd]`. */
  warn(...data: any[]): void;
  error(...data: any[]): void;
  trace(...data: any[]): void;
}
declare var console: Console;

/** A timer: pass it to clearTimeout / clearInterval. */
interface FlowlangTimer { readonly __flowlangTimer: never }
/** Runs on the engine's event loop: while a call awaits, or between calls. */
declare function setTimeout<A extends any[]>(callback: (...args: A) => void, delay?: number, ...args: A): FlowlangTimer;
declare function setInterval<A extends any[]>(callback: (...args: A) => void, delay?: number, ...args: A): FlowlangTimer;
declare function clearTimeout(timer?: FlowlangTimer): void;
declare function clearInterval(timer?: FlowlangTimer): void;
declare function queueMicrotask(callback: () => void): void;

/** Calls into any command - Rust, Python, flow or JavaScript. */
declare namespace flowlang {
  /** Thrown when a called command fails (raises, panics or returns an error envelope). */
  class CommandError extends Error {}
  /**
   * Runs lib:ctl:cmd with named arguments and returns its value. Throws
   * ReferenceError for an unknown command, TypeError for arguments that do
   * not fit its declared parameters, and CommandError when it fails. A
   * JavaScript command that awaits returns a Promise.
   */
  function call(lib: string, ctl: string, cmd: string, args?: Record<string, any>): any;
  /** flowlang.api.<lib>.<ctl>.<cmd>(...positional) == flowlang.call(lib, ctl, cmd, {...}). */
  const api: FlowlangApi;
}

// ---- web platform APIs (src/jsweb.js) ----
// A fetched body is read whole; there are no streams, Blob or FormData.

declare class DOMException extends Error {
  constructor(message?: string, name?: string);
}
declare class Event {
  constructor(type: string);
  readonly type: string;
  readonly defaultPrevented: boolean;
  target: EventTarget | null;
  preventDefault(): void;
}
type EventListenerLike = ((event: Event) => void) | { handleEvent(event: Event): void };
declare class EventTarget {
  addEventListener(type: string, listener: EventListenerLike, options?: { once?: boolean } | boolean): void;
  removeEventListener(type: string, listener: EventListenerLike): void;
  dispatchEvent(event: Event): boolean;
}
declare class AbortSignal extends EventTarget {
  private constructor();
  readonly aborted: boolean;
  readonly reason: any;
  onabort: ((event: Event) => void) | null;
  throwIfAborted(): void;
  static abort(reason?: any): AbortSignal;
  static timeout(ms: number): AbortSignal;
  static any(signals: Iterable<AbortSignal>): AbortSignal;
}
declare class AbortController {
  readonly signal: AbortSignal;
  abort(reason?: any): void;
}

declare class TextEncoder {
  readonly encoding: "utf-8";
  encode(input?: string): Uint8Array;
  encodeInto(input: string, dest: Uint8Array): { read: number; written: number };
}
declare class TextDecoder {
  /** utf-8 only. */
  constructor(label?: string, options?: { fatal?: boolean; ignoreBOM?: boolean });
  readonly encoding: "utf-8";
  readonly fatal: boolean;
  readonly ignoreBOM: boolean;
  decode(input?: ArrayBuffer | ArrayBufferView): string;
}
declare function atob(data: string): string;
declare function btoa(data: string): string;
declare var crypto: {
  getRandomValues<T extends Int8Array | Uint8Array | Uint8ClampedArray | Int16Array | Uint16Array | Int32Array | Uint32Array | BigInt64Array | BigUint64Array>(array: T): T;
  randomUUID(): string;
};
declare function structuredClone<T>(value: T, options?: { transfer?: any[] }): T;
declare var performance: { now(): number; readonly timeOrigin: number };

declare class URLSearchParams implements Iterable<[string, string]> {
  constructor(init?: string | URLSearchParams | Record<string, string> | Iterable<[string, string]>);
  readonly size: number;
  append(name: string, value: string): void;
  delete(name: string, value?: string): void;
  get(name: string): string | null;
  getAll(name: string): string[];
  has(name: string, value?: string): boolean;
  set(name: string, value: string): void;
  sort(): void;
  forEach(callback: (value: string, key: string, parent: URLSearchParams) => void, thisArg?: any): void;
  entries(): IterableIterator<[string, string]>;
  keys(): IterableIterator<string>;
  values(): IterableIterator<string>;
  [Symbol.iterator](): IterableIterator<[string, string]>;
  toString(): string;
}
declare class URL {
  constructor(url: string | URL, base?: string | URL);
  static canParse(url: string | URL, base?: string | URL): boolean;
  static parse(url: string | URL, base?: string | URL): URL | null;
  href: string;
  readonly origin: string;
  protocol: string;
  username: string;
  password: string;
  host: string;
  hostname: string;
  port: string;
  pathname: string;
  search: string;
  hash: string;
  readonly searchParams: URLSearchParams;
  toString(): string;
  toJSON(): string;
}

type HeadersInit = Headers | Record<string, string> | Iterable<[string, string]>;
declare class Headers implements Iterable<[string, string]> {
  constructor(init?: HeadersInit);
  append(name: string, value: string): void;
  delete(name: string): void;
  get(name: string): string | null;
  getSetCookie(): string[];
  has(name: string): boolean;
  set(name: string, value: string): void;
  forEach(callback: (value: string, key: string, parent: Headers) => void, thisArg?: any): void;
  entries(): IterableIterator<[string, string]>;
  keys(): IterableIterator<string>;
  values(): IterableIterator<string>;
  [Symbol.iterator](): IterableIterator<[string, string]>;
}
type BodyInit = string | URLSearchParams | ArrayBuffer | ArrayBufferView;
interface Body {
  readonly bodyUsed: boolean;
  /** Always null: bodies are read whole, with the methods below. */
  readonly body: null;
  arrayBuffer(): Promise<ArrayBuffer>;
  bytes(): Promise<Uint8Array>;
  text(): Promise<string>;
  json(): Promise<any>;
}
interface RequestInit {
  method?: string;
  headers?: HeadersInit;
  body?: BodyInit | null;
  redirect?: "follow" | "manual" | "error";
  signal?: AbortSignal | null;
}
declare class Request implements Body {
  constructor(input: string | URL | Request, init?: RequestInit);
  readonly url: string;
  readonly method: string;
  readonly headers: Headers;
  readonly signal: AbortSignal | null;
  readonly redirect: "follow" | "manual" | "error";
  readonly bodyUsed: boolean;
  readonly body: null;
  arrayBuffer(): Promise<ArrayBuffer>;
  bytes(): Promise<Uint8Array>;
  text(): Promise<string>;
  json(): Promise<any>;
  clone(): Request;
}
interface ResponseInit { status?: number; statusText?: string; headers?: HeadersInit }
declare class Response implements Body {
  constructor(body?: BodyInit | null, init?: ResponseInit);
  static json(data: any, init?: ResponseInit): Response;
  static error(): Response;
  static redirect(url: string | URL, status?: number): Response;
  readonly status: number;
  readonly statusText: string;
  readonly ok: boolean;
  readonly headers: Headers;
  readonly url: string;
  readonly redirected: boolean;
  readonly type: "basic" | "default" | "error";
  readonly bodyUsed: boolean;
  readonly body: null;
  arrayBuffer(): Promise<ArrayBuffer>;
  bytes(): Promise<Uint8Array>;
  text(): Promise<string>;
  json(): Promise<any>;
  clone(): Response;
}
/**
 * HTTP(S) through the OS trust store and the *_PROXY variables. An HTTP error
 * status is a Response (check `ok`); a network failure rejects with TypeError.
 */
declare function fetch(input: string | URL | Request, init?: RequestInit): Promise<Response>;
