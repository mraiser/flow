// The globals a JavaScript command sees in flowlang's engine (javascript_runtime),
// beyond the ECMAScript standard library. Written by flowb next to the generated
// command sources; check_js type-checks command bodies against it. Nothing else is
// defined: no fetch, require, process or DOM.

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
