# Flowlang

A Rust implementation of the **Flow language** — a dataflow-oriented programming
language designed for visual "flow" diagrams, loosely based on
[Prograph](https://en.wikipedia.org/wiki/Prograph). Flowlang executes Flow
programs (defined in JSON), and provides a unified function-call interface
across **Rust, Python, JavaScript, and Java**, so a single dataflow graph can
mix nodes written in any of them. It also speaks the
**[Model Context Protocol](https://modelcontextprotocol.io)** out of the box:
the `flowmcp` binary exposes every command as an MCP tool that any MCP client
can call.

> **Note:** Support for back-end commands written in Java is kind of broken
> for now. Contact me if you'd like to help fix it. JavaScript and TypeScript
> commands work (see *Language runtimes*).

A Flow program is a directed graph of operations ("commands") in which data
flows along connections between nodes. Using an IDE like
[Newbound](https://github.com/mraiser/newbound), a developer draws a diagram of
how data moves through functions and conditions; Flowlang interprets the JSON
representation of that diagram, firing each operation as its inputs become
ready. Commands may be implemented in Flow itself or natively in Rust, Python,
JavaScript, or Java — the runtime dispatches to the right language engine and
moves data in and out. Language runtimes persist between calls, so a Python
interpreter's globals or a JS engine's state survive from one invocation to the
next.

## Relation to ndata

Flowlang is built on the companion crate
[ndata](https://crates.io/crates/ndata), which provides the dynamic data
structures used to pass data between flow nodes: `Data`, `DataObject`, and
`DataArray` behave like loosely-typed JSON values. ndata implements an internal
heap with reference counting and *manual* garbage collection, which lets
Flowlang create and share dynamic data across threads and languages without
fighting the borrow checker — much like a garbage-collected language embedded
in Rust.

Two practical consequences:

- **Never wrap ndata types in `Arc` or `Mutex`.** They are internally
  thread-safe (atomic refcounts and locks); wrapping them adds nothing and can
  keep the GC from ever collecting them.
- **Call `DataStore::gc()` periodically** in long-running services. Collection
  is manual by design; nothing is freed until you ask.

> Flowlang 0.3.30+ should be used with **ndata 0.3.17 or later** — earlier
> ndata versions had a JSON parser bug that rejected negative numbers, which
> made flow diagrams containing nodes at negative coordinates unreadable.

## MCP: every command is a tool

The `flowmcp` binary is a Model Context Protocol server speaking JSON-RPC over
stdin/stdout. It implements `initialize`, `tools/list`, `tools/call`,
`prompts/list`, and `resources/list`. Tools are named
`library-control-command`, and `tools/call` invokes the corresponding Flow,
Rust, Python, or JavaScript command directly.

A real session (run from a directory containing a `data/` folder; note that
requests must carry a numeric `id`):

```
→ {"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"demo","version":"0"}},"id":1}
← {"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"newbound-mcp","version":"0.1.0"},"capabilities":{"tools":{"listChanged":false}},"protocolVersion":"2024-11-05"}}

→ {"jsonrpc":"2.0","method":"tools/list","params":{},"id":2}
← {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"flowlang-data-read", ...}, ...]}}

→ {"jsonrpc":"2.0","method":"tools/call","params":{"name":"testflow-testflow-test_add","arguments":{"a":300,"b":120}},"id":3}
← {"jsonrpc":"2.0","id":3,"result":{"content":[{"text":"300+120=420","type":"text"}]}}
```

This makes Flowlang a natural substrate for LLM tooling: the majority of LLM
SDKs are Python-based, and Flowlang's embedded Python (via PyO3) runs them as
flow nodes with state preserved between calls; performance-critical steps can
be native Rust commands; and the whole orchestration — prompt construction,
model calls, branching on responses — is a visual dataflow graph rather than a
monolithic script. `flowmcp` then exposes the finished agent to any MCP client
with no additional glue. Flows can also be served over HTTP or invoked from
the command line.

## Installation

Three binaries ship with the crate: `flow` (the interpreter), `flowb` (the
builder for Rust and Python commands), and `flowmcp` (the MCP server).

```bash
# From crates.io:
cargo install flowlang

# Or from source:
git clone https://github.com/mraiser/flow.git
cd flow
cargo build --release
```

All three binaries look for a `data/` directory in the current working
directory containing the flow libraries. The repository (and the published
crate) include a `data/` folder with an example library, **testflow**.

## Command-line usage

```bash
cd path/to/flow   # any directory containing a 'data' folder

flow testflow testflow test_add <<< '{"a": 300, "b": 120}'
```

Output:

```json
{"a":"300+120=420"}
```

The arguments are `<library> <control> <command>`, with the JSON input on
stdin and the result printed to stdout as JSON. When the flow fails, or the
command or its input cannot be found or read, stdout is an error envelope
instead and the exit status is 1:

```json
{"status":"err","msg":"flow failed (panic) at op 0 primitive \"+\": list input 'a' is not an array","flow_error":{"code":"panic","msg":"list input 'a' is not an array","at":[{"case":0,"op":0,"iter":null,"type":"primitive","name":"+"}],"cause":null}}
```

(A Rust, Python or JavaScript command that fails returns its own
`{"a": {"status": "err", ...}}` result, and the exit status stays 0.)

`flowb` regenerates source for Rust and Python commands from the `data/`
definitions:

```bash
flowb            # build everything, then regenerate the typed Rust API
flowb ALL        # same as above
flowb API        # regenerate only the typed Rust API
flowb <library> <control> <command>   # build a single command
```

## Using Flowlang as a library

### Single-crate layout

The simplest structure hosts flow libraries directly in your crate: set
`"root": "."` in each library's `data/<library>/meta.json`, and `flowb` will
generate command sources into `src/<library>/…` of the current crate. This
repository itself is organized this way.

```rust
mod generated_initializer;

fn main() {
    // Initialize the runtime and register all generated commands.
    generated_initializer::initialize_all_commands(flowlang::init("data"));

    // ... your application logic ...
}
```

(`flowlang::init` returns the ndata configuration tuple that
`initialize_all_commands` needs — pass it straight through.)

### Workspace layout with sub-crates

Larger projects can split libraries into separate crates. Give each library a
`"root"` naming its sub-crate directory: `{"root": "core-libs", ...}`.
A library whose `meta.json` has **no** `root` field defaults to a sub-crate
named `cmd`.

```toml
# /my_project/Cargo.toml
[package]
name = "my_project_bin"
# ...

[workspace]
members = ["core-libs", "command-libs", "ffi-lib"]
```

Running `flowb` from the project root then creates the sub-project
directories, generates each one's `lib.rs`, `cmdinit.rs`, and `Cargo.toml`,
assembles the flow libraries as modules within their designated crates,
generates `src/generated_initializer.rs` for the main binary, and adds path
dependencies to the top-level `Cargo.toml`.

**FFI sub-crates: dynamic loading and hot reload.** A sub-crate can be
compiled as a dynamic shared library by setting `"ffi": true` in the `cargo`
section of the library's `meta.json`. This is not just a build setting — it is
a runtime plugin system:

- The generated sub-crate exports a C-ABI entry point
  (`#[no_mangle] pub unsafe extern "C" fn mirror_<crate>(...)`).
- The host's `generated_initializer.rs` loads the library at runtime with
  [libloading](https://crates.io/crates/libloading) — from a uniquely-named
  temp copy, so rebuilding the sub-crate and reloading picks up the new code
  without restarting the host. This is how Newbound hot-loads command crates.
- On load, the plugin **mirrors the host's ndata heap**, so `DataObject`s
  cross the FFI boundary as shared handles — no serialization. The commands it
  registers are function pointers into the dylib; invoking them is a direct
  FFI call.
- Because each shared library links its own dependencies, two sub-projects can
  depend on **conflicting versions of the same crate**.

The host binary needs `libloading` in its own `[dependencies]`; no `build.rs`
or linker configuration is required.

### The generated typed API

`flowb` also generates a typed Rust API (`src/api.rs` in each sub-crate) so
commands can be called like ordinary functions:

```rust
// Current form: modules of stateless functions
let sum = api::testflow::testflow::test_add(300, 120);
```

As of 0.3.30 the previous struct-based form
(`api::new().testflow.testflow.test_add(...)`) still works but is deprecated;
each method's deprecation warning names its replacement path. The struct
facade will be removed in a future release.

## HTTP service

Flowlang includes a small HTTP server that exposes flow commands as web
endpoints:

```bash
flow flowlang http listen <<< '{"socket_address": "127.0.0.1:7878"}'
```

Any command can then be invoked via HTTP GET. Query parameters are decoded
with `hex_decode`, which (as of 0.3.30) is byte-for-byte compatible with
JavaScript's `encodeURIComponent` output, including multi-byte UTF-8;
`hex_encode` is its exact inverse.

## Language runtimes

Language engines are feature-gated so the base crate stays light. Enable the
ones your flows need:

```bash
cargo build --features python_runtime      # embeds Python 3.10-3.14 via PyO3
cargo build --features javascript_runtime  # embeds V8 via deno_core
cargo build --features java_runtime        # loads a JVM via JNI
```

- **Python** — commands run in a persistent embedded interpreter. Arguments
  arrive as `NDataObject` / `NDataArray` / `NDataBytes`, thin Python wrappers
  around the underlying Rust data — no serialization on the boundary, and
  Python mutates the same data Rust sees. If a `venv` directory exists next
  to `data/`, it is activated automatically. A Python command returns
  exactly what a Rust command returns — `{"a": value}`, or
  `{"a": {"status": "err", "msg": ...}}` on failure — so it works anywhere a
  Rust command does, including as a flow op. The return value converts
  exactly or not at all:

  | Python | ndata |
  |---|---|
  | `None`, `bool`, `str` | `DNull`, `DBoolean`, `DString` |
  | `int` (incl. `IntEnum`, numpy ints) | `DInt`; outside 64-bit range is an `OverflowError` |
  | `float` (incl. numpy floats, `Fraction`) | `DFloat` |
  | `bytes`, `bytearray` | `DBytes` |
  | `dict` with `str` keys | `DObject` |
  | `list`, `tuple` | `DArray` |
  | `NDataObject` / `NDataArray` / `NDataBytes` | the same handle |

  Anything else (`set`, `Decimal`, `datetime`, …) is a `TypeError` that
  names an explicit conversion. A `FLAT` command must return a `dict` or
  `NDataObject`. An exception's `msg` names the exception, its location in
  the command body (`body line N`, or `imports line N`), and the full
  traceback. A declared parameter missing from the call is an error.

  Each command is the module `lib.ctl.cmd` under its library's package root,
  which is appended to the END of `sys.path` — so a command imports a sibling
  as `from lib.ctl import other`, and a library named like an existing Python
  module (`email`, `html`, …) fails with an error naming that module rather
  than shadowing it. Neither the working directory nor a command's own
  directory is on `sys.path`. A `venv` is configured once, at interpreter
  start (its `sys.path` and `.pth` files, so editable installs work).

  `flow pytest [pytest arguments]` runs pytest inside the embedded runtime
  (pytest must be installed in that Python, e.g. the venv next to `data/`).
  Tests are ordinary pytest files with every library's package root on
  `sys.path`, so they can call any command, import a command's module and
  call its function directly, and use real `ndata_py` objects:

  ```python
  import flowlang, pytest
  from testflow.testflow import test_python

  def test_add():
      assert flowlang.api.testflow.testflow.test_add(1, 2) == "1+2=3"

  def test_hello():
      assert test_python.test_python("you") == "Hello you from python"

  def test_missing_argument():
      with pytest.raises(flowlang.CommandError, match="missing required parameter"):
          flowlang.call("testflow", "testflow", "test_python")
  ```

  An application embedding flowlang can offer the same with
  `flowlang::pyenv::run_pytest(args)`.

  Generated command functions are annotated from the declared types —
  `def add(a: "int", items: "NDataArray") -> "Mapping[str, Any]":` — and
  flowb writes type stubs for `ndata_py` and `flowlang` (`ndata_py.pyi`,
  `flowlang.pyi`) into the package root next to the generated sources, so
  pyright, mypy and editors check and complete command bodies. Declared
  types map as String → `str`, Integer → `int`, Float/Double → `float`,
  Boolean → `bool`, JSONObject → `NDataObject`, JSONArray → `NDataArray`,
  InputStream → `NDataBytes`, anything else → `Any`; return types accept
  what a command may return (`Mapping[str, Any]` for JSONObject and FLAT,
  `Sequence[Any]` for JSONArray). The annotations are strings, never
  evaluated at runtime.

  Output from Python commands is labelled with the command and written as
  complete lines straight to the process's streams, so it appears
  immediately and in order with the server's own output, even when stdout
  is not a terminal:

  ```
  [py testflow:testflow:report] building totals
  [py testflow:testflow:report] WARNING mylib.db: slow query (2.3 s)
  ```

  `print()` goes to stdout; `sys.stderr` and `logging` go to stderr (in
  subprocess mode, everything goes to stderr: the child's stdout carries the
  result). Unless a command configures logging itself, the root logger gets
  a `LEVEL name: message` handler at Python's default WARNING level; set
  `FLOWLANG_PYTHON_LOG_LEVEL=INFO` (or `DEBUG`, …) to change it. Threads a
  command starts write with a plain `[py]` label (or the command's, where
  new threads inherit the context — free-threaded 3.14's default).

  A CPython bundled next to the executable, in `<exe dir>/python` (a
  relocatable python-build-standalone build, linked with an
  `$ORIGIN/python/lib` rpath), is used automatically: the runtime sets
  `PYTHONHOME` to it unless the environment sets one, and the subprocess
  mode and the checker prefer its `python3`. This is how a binary can ship
  with its own Python.

  Concurrency: with a standard CPython, Python commands share the GIL —
  CPU-bound commands run one at a time (the lock is released while
  `flowlang.call` runs another command, so a slow callee blocks no one).
  Built against a free-threaded CPython (3.13t/3.14t:
  `PYO3_PYTHON=python3.14t cargo build --features python_runtime`), Python
  commands run in parallel: `ndata_py` and `flowlang` declare that they do
  not need the GIL, and four CPU-bound commands on four cores finish about
  3.6× faster than one after another. A third-party extension module that
  does not support free-threading re-enables the GIL when imported (Python
  warns when it does).

  Python commands hot-reload: when a command's generated file changes
  (`flowb`, or newbound's `dev.code` edits, regenerate it), the next call
  reloads the module in place (`importlib.reload`) before running it — no
  restart. Module-level code runs again, so top-level assignments are
  re-made, while names it no longer assigns keep their old values. If the
  new source fails to load (a `SyntaxError`, say), each call reports that
  error until it is fixed; the old code never runs silently. A module that
  imports another command's module sees the new code once that command has
  been called (or reloaded); third-party packages are not reloaded.

  The embedded interpreter provides two modules, mirroring the Rust crates:

  ```python
  import ndata_py   # NDataObject, NDataArray, NDataBytes, NDataPyError, init, gc
  import flowlang   # call, CommandError

  o = ndata_py.NDataObject()
  o["total"] = flowlang.call("testflow", "testflow", "test_add", a=300, b=120)
  ```

  `flowlang.call(lib, ctl, cmd, *args, **kwargs)` runs any command — Rust,
  Python, flow, JavaScript or Java — with arguments cast to the declared
  parameter types (positional arguments in declared order), and returns its
  value (a flow's single output, or all of its outputs as an `NDataObject`).
  `flowlang.api.<lib>.<ctl>.<cmd>(...)` is the same call as an attribute
  path, mirroring the Rust `api` (names with `-` are written with `_`):

  ```python
  flowlang.api.testflow.testflow.test_add(300, 120)   # "300+120=420"
  ```

  flowb types the whole tree in `flowlang.pyi` from the libraries in
  `data/`, so editors complete every command and type checkers check its
  arguments and result. It raises `LookupError` for an unknown command,
  `TypeError` for an argument that cannot be cast, and `flowlang.CommandError`
  when the command fails — including a result whose `status` is `"err"`. The
  interpreter lock is released while the command runs. In subprocess mode
  `ndata_py` is absent and `flowlang.call` raises a `RuntimeError` saying the
  embedded runtime is required.

  The wrappers behave like the built-in types they stand for, while
  reading and writing the shared ndata store directly:

  - `NDataObject` is a `MutableMapping` (registered with `collections.abc`):
    `[]`, `in`, `len`, iteration over keys, live `keys()`/`values()`/`items()`
    views, `get`, `pop`, `popitem`, `setdefault`, `update`, `clear`, `copy`
    (shallow), and `==` with any `dict` or mapping. Keys must be `str`.
  - `NDataArray` is a `MutableSequence`: indexing and slicing (including
    negative steps), `in`, `index`, `count`, `append`, `extend`, `+=`,
    `insert`, `remove`, `pop`, `reverse`, `reversed()`, `clear`, `copy`, and
    `==` with any `list`.
  - `NDataBytes` supports `bytes(b)` and `==` with `bytes`/`bytearray`.
  - `to_dict()` / `to_list()` / `ndata_py.to_py(x)` make deep native copies,
    for code that requires a real `dict` or `list`. `json.dumps` accepts only
    real dicts and lists, so serialize with
    `json.dumps(x, default=ndata_py.to_py)` (or `x.to_json()`, ndata's own
    JSON, which also encodes bytes).
  - Like `dict` and `list`, the wrappers are unhashable, and their `repr`
    shows the contents: `NDataObject({'a': 1})`.

  Without `python_runtime`, Python commands run in a `python3` subprocess
  (the `venv` interpreter if present) with the same envelope, conversion
  rules and error reports; the difference is that data crosses by value as
  JSON — arguments arrive as plain `dict`/`list`, and bytes as ndata's hex
  string.
- **JavaScript** — commands run in one embedded V8 isolate (`deno_core`),
  owned by a thread of its own: callers on any thread hand their call to it
  and wait. A command record's language is `"js"` or `"javascript"`
  (newbound's `dev.code` writes the latter). The body is the body of a
  function whose parameters are the declared params, in order (each must be
  a JavaScript identifier); a body that uses `await` is an `async` function.
  A declared param missing from the call is an error. A JavaScript command
  returns exactly what a Rust command returns — `{"a": value}`, or
  `{"a": {"status": "err", "msg": ...}}` on failure — so it works anywhere a
  Rust or Python command does.

  Data is live, as for Python: a `JSONObject` or `JSONArray` argument — and
  every object or array read from one — is a handle on the caller's own
  ndata, so changes are seen by the caller and by any Rust or Python command
  holding it, with no copying. A handle behaves like a plain object or array
  (property access, `in`, `delete`, `Object.keys`, spread, `for…of`,
  `JSON.stringify`; for arrays `Array.isArray`, `length`, indexing and the
  `Array` methods). Writing a plain object or array stores new ndata (as a
  `dict` does from Python); writing a handle stores the same object;
  `toJSON` (Date, URL) is honoured; `undefined` removes an object's key; bytes
  (`InputStream`) cross as `Uint8Array`. Returning a handle returns the same
  object. A function, symbol, BigInt, `Map`, `Set`, Promise or cyclic
  structure is a TypeError that names it, and a `FLAT` command must return an
  object. Each read makes a new handle, so compare contents, not identity;
  `ndata.isHandle`, `ndata.toJS` (a plain deep copy), `ndata.object()` and
  `ndata.array()` are the global `ndata`'s helpers. A command that returns a
  Promise answers with what it resolves to; a rejection is its error.

  The engine works like Node's event loop: a call's synchronous part runs to
  completion, and a call that awaits leaves the engine free for other calls
  while it waits, so many awaiting calls progress together. `setTimeout`,
  `setInterval`, `clearTimeout`, `clearInterval` and `queueMicrotask` exist;
  timers a command leaves behind keep running between calls, and global
  state persists between calls. A promise that can never settle is an
  error, not a hang. Every call has a time limit — 30 s, or
  `FLOWLANG_JS_TIMEOUT_MS` (`0`: none), or `jscmd::set_default_timeout` —
  covering its synchronous part and its awaiting; JavaScript running past
  it is stopped and the engine carries on. (Stopping JavaScript discards the
  microtasks V8 had queued behind it; an awaiting call that loses its work
  that way says so.)

  `flowlang.call(lib, ctl, cmd, {name: value})` and
  `flowlang.api.<lib>.<ctl>.<cmd>(positional...)` run any command — Rust,
  Python, flow or JavaScript — and return its value, as Python's
  `flowlang.call` does (handles passed along are the same objects for the
  callee); they throw `ReferenceError` for an unknown command,
  `TypeError` for arguments that do not fit, and `flowlang.CommandError` when
  the command fails. A JavaScript callee runs in the same isolate and returns
  its value as is (a Promise if it is async). Any other callee runs on a
  worker thread while the caller waits, so it can call JavaScript in turn —
  JavaScript → Rust, Python or flow → JavaScript, to any depth: the inner
  command runs in the same isolate (same globals), its exception reaches the
  outer one as the middle command's `CommandError`, and the outer call's time
  limit covers it all. The inner command must finish without the event loop
  (the middle command is waiting for it): one that awaits fails, saying so.

  An exception's `msg` is the exception, its location in the command body
  (`body line N`) and the stack, in the body's own line numbers. A syntax
  error is reported on every call until the body is fixed — the old code
  never runs silently — and a panic inside the engine restarts it rather
  than disabling JavaScript. `console.log`/`info`/`debug` write to stdout
  (stderr under `flowmcp`, whose stdout is the JSON-RPC channel) and
  `console.warn`/`error`/`trace` to stderr, as complete lines labelled with
  the command: `[js testflow:testflow:report] building totals` (a timer's
  output carries the label of the command that set it, whenever it fires).

  `jscmd::check(lib, id)` checks a command without running it: a V8 syntax
  check (esbuild's bundle, for a command with imports or in TypeScript),
  plus a TypeScript type check when `tsc` is found (`$FLOWLANG_TSC`,
  `node_modules/.bin/tsc` next to `data/`, or `tsc` on the PATH) — the body
  as `// @ts-check` JavaScript with JSDoc types from its declared params,
  against `flowlang.d.ts`, so names the engine does not provide (`fetch`,
  `require`) are errors and `flowlang.api` calls are checked against the
  callee. Problems are reported at body lines; `check_quick` is the syntax
  check alone. `flowb` writes `flowlang.d.ts` (the engine's globals and a
  typed `flowlang.api` tree of every command in the store) and each command
  as `<lib>/<ctl>/<cmd>.js` into the library's crate `src/`, for editors.

  `flow jstest [--no-check] [files or directories]` runs `*.test.js` and `*.test.ts` files in the same
  engine, so tests call any command; each file gets `test(name, fn)`
  (`fn` may be async; `test.skip`) and `assert` (`equal`, `notEqual`,
  `deepEqual`, `throws`, `rejects`). Exit status: 0 all passed, 1 a failure,
  2 nothing ran. A test file may start with `import` statements — npm
  packages, files beside it (`./fixtures.js`), and commands' modules
  (`flowlang:<lib>/<ctl>/<cmd>`, to test a command's helpers directly) —
  and then runs from an esbuild bundle, with its lines, top-level `await`
  and `test`/`assert` unchanged; an import that does not resolve fails the
  file at its own line. A `*.test.ts` file is TypeScript and always runs
  that way, so it needs esbuild.

  Before anything runs, tsc type-checks every `*.test.ts` file (strictly)
  and every `*.test.js` that starts with `// @ts-check` (as JavaScript), in
  one run, in place: against `flowlang.d.ts` (so `flowlang.api` calls are
  checked against the callees' declared parameters), with `test`/`assert`
  typed (`assert.equal`'s two sides must be comparable), top-level `await`
  allowed, and `flowlang:` imports resolved to their commands. A file with
  type errors is reported at its lines and its tests do not run; errors in
  a helper file the tests import are reported under its name, while an
  imported command's are left to its own check. `--no-check` skips this.
  tsc is found as for `jscmd::check`; without it, a note says the files
  were not checked and the tests run.

  Web platform APIs are built in: `fetch` (with `Headers`, `Request`,
  `Response` and `AbortController`; requests run off the engine thread, TLS
  against Mozilla's roots or `SSL_CERT_FILE`'s certificates, `*_PROXY`/
  `NO_PROXY` honoured, bodies read whole and not decompressed; a small
  interim client, `ureq` with rustls, behind one function), `URL`/`URLSearchParams`,
  `TextEncoder`/`TextDecoder`, `atob`/`btoa`, `crypto.getRandomValues` /
  `randomUUID`, `structuredClone`, `performance.now`, `EventTarget` and
  `DOMException`. Node's built-ins (`fs`, `process`, `Buffer`, `require`) are
  not there.

  **Imports and npm packages.** A command's `import` field holds ES `import`
  statements; packages resolve from `node_modules` next to `data/` (`npm
  install lodash` there), CommonJS and ESM alike. Such a command runs from
  an esbuild bundle: `flowb` (or the first call) writes the command as
  `<lib>/<ctl>/<cmd>.js` — its imports, then the body as the module's
  default export — and bundles it into `<cmd>.bundle.js`, rebuilt when the
  command, its imports or the npm lockfile's content change. esbuild is
  found as `$FLOWLANG_ESBUILD` (alone, when set), else
  `node_modules/.bin/esbuild` next to `data/`, or `esbuild` on the PATH
  (`npm install esbuild`).

  Bundles are build output, like compiled Rust: `flowb BUNDLES` builds every
  one a store needs (and fails if any cannot be built), and a deployment
  that ships them next to `data/`, at the same paths, needs neither esbuild
  nor `node_modules` — a bundle built from the command's current code is
  used as is when there is no lockfile to contradict it. A bundle built from
  other code is never run: without esbuild to rebuild it, the command
  reports that it needs one. Problems surface before anything runs — a missing package, or one
  that needs Node's built-ins, is an error at its `imports line N` — and
  errors at run time point into the author's code through the bundle's
  source map (`body line N`, `flowlang:<lib>/<ctl>/<cmd>.js:L:C`) and into
  packages as `npm:<path>`.

  **Sharing code between commands.** A command imports another JavaScript or
  TypeScript command, in any library, as a module — the counterpart of a
  Python command's `from lib.ctl import other`:

  ```js
  import format, { slugify } from "flowlang:mylib/text/format";
  ```

  The default export is the command's function (called directly: no casting,
  no envelope), and whatever the imported command's own `import` field
  `export`s comes with it, so a command can be a library of helpers as well
  as a command. Imports chain, and JavaScript and TypeScript import each
  other. An edit to an imported command rebuilds the bundles of the commands
  that import it, and tsc (`jscmd::check`) checks types across the import;
  an imported command's own type errors are its own check's to report. A
  frame inside one reads `flowlang:<lib>/<ctl>/<cmd>.js:L:C`. Each importing
  command bundles its own copy, so module-level state is per importing
  command; state shared between commands belongs in ndata. An import that
  names no JavaScript or TypeScript command is an error at its
  `imports line N` — a Rust, Python or flow command is called with
  `flowlang.call` instead.

  **TypeScript.** A command whose language is `"typescript"` (or `"ts"`) is
  TypeScript: the same body, params typed from the declarations
  (`<cmd>.ts`), compiled by esbuild — so TypeScript commands need esbuild
  too. esbuild does not type-check; `jscmd::check` runs tsc on them in
  strict mode.
- **Java** — requires the Java helper classes on the classpath and `libjvm`
  on the library path (e.g. `LD_LIBRARY_PATH`).
- **Rust** — statically linked by default: `flowb` generates a wrapper and
  registry entry, and commands compile into your binary or its sub-crates. A
  sub-crate marked `"ffi": true` instead becomes a dynamic library that Rust
  applications load and invoke over FFI at runtime, with hot reload — see
  *FFI sub-crates* under [Using Flowlang as a library](#using-flowlang-as-a-library).

## Design notes

- **Interpreter:** the heart of Flowlang is a two-phase event loop. An
  *operation pass* fires every operation whose inputs are satisfied; a
  *connection pass* propagates outputs along connections. The passes repeat
  until the graph is quiescent — a topological sort performed on the fly. A
  single flow executes on one thread, but ndata's thread-safety means separate
  `Command::execute` calls can safely run concurrently on different threads.
- **Stepper and fast path:** `flowlang::stepper` defines execution one step at
  a time, as events over a frame stack kept in ndata, so a debugger can pause,
  inspect and resume a run. `Code::execute` takes the same steps through
  `flowlang::fastrun`, which keeps its bookkeeping native and builds no
  events. `tests/fastrun_parity.rs` checks that the two agree on results,
  side effects, errors and step counts, over the testflow commands and
  thousands of generated flows. `tests/fastrun_speed.rs` checks that
  `Code::execute` stays at least 3x faster than the stepper.
- **Step budget:** a run that takes more than 1,000,000 steps ends with an
  error ("did not finish within 1000000 steps"), so an open loop cannot run
  forever. A step is one wire, synchro, op evaluation or loop iteration
  (roughly). A long but legitimate loop can reach it too: testflow's
  `test_speed` loop (five ops, 17 steps a pass) stops after about 58,800
  passes.
- **Synchros:** a case may carry `syncs: [{"src": i, "dest": j}]`, data-less
  edges between two of its operations meaning "op `i` finishes before op `j`
  starts". An operation with an incoming synchro never runs at case start;
  it runs, exactly once, when every awaited input has landed AND the source of
  every incoming synchro is done (a list/loop source after its last
  iteration, a local after its whole local case). A case ends only when every
  connection and every synchro has fired; if it ends early (`next`,
  `terminate`, `fail`) a pending destination never runs. Order among
  operations that are ready together stays unspecified, so any ordering a
  flow depends on must be a synchro. The key is optional (absent = none), and
  endpoints that are not two distinct operations of the case are inert.
  Wires plus synchros must be acyclic: a cycle stalls the case.
- **Robustness:** generated Rust command wrappers validate declared parameters
  before use and run the command body inside a panic guard. A missing or
  mistyped argument produces a structured error response — it cannot take down
  the process, which matters when commands are compiled into hot-loaded
  dynamic libraries where an escaped panic would abort.
- **Control flow:** the interpreter propagates Flow-level events ("fail",
  "terminate", "next case") as a `CodeException` enum through `Result`,
  keeping the core loop free of deeply nested conditionals.
- **Errors say what and where:** a failed flow is
  `Err(CodeException::Fail(FlowError))`. A `FlowError` has a `code` ("fail"
  for a `fail` rule, "stall" for a stuck case, "I-4", "absent-output",
  "panic", "depth" or "budget") and a `msg`. It also has `at`, the path of
  sites from the root frame down to the failure: each site is the case, the
  op, its type and name, and the iteration of a list or loop op. A `fail`
  rule that fired because a local or flow command op failed carries that
  failure as its `cause`. `to_string()` gives one line:

  ```
  flow failed (fail) at op 0 local "check": its fail rule fired; because flow failed (fail) at op 0 local "check" > op 0 match "ok": its fail rule fired
  ```

  `to_envelope()` gives the `{"status": "err", "msg", "flow_error"}` object
  that the CLI, `flowmcp`, `flowlang.call` and the HTTP listener report. A
  failure is returned, never printed. `Command::try_cast_params` checks
  arguments against the declared types and names the one that does not fit
  (`parameter 'b' of testflow:test_add must be an Integer, not "x7"`).
  `cast_params` panics with the same message.
- **Code generation over macros:** `flowb` writes ordinary Rust source
  (command wrappers, registries, the typed API) rather than using procedural
  macros — what runs is readable code checked into your tree.
- **Vendored crypto:** peer-to-peer session encryption uses a vendored X25519
  key agreement (`flowlang::x25519`) and a vendored AES-256 block cipher
  (`flowlang::aes`: encrypt/decrypt single blocks or block-aligned ECB runs;
  no padding, IV or authentication — modes are the caller's job), and content
  hashing a vendored BLAKE2b (`flowlang::blake2b`: digests of 1-64 bytes,
  keyed mode, salt and personalization) — no external crypto dependency
  chain. All three are verified against their standards' test vectors
  (RFC 7748, FIPS 197 / SP 800-38A, RFC 7693) in this crate's test suite.
  The AES runs on the CPU's AES instructions where they exist (x86-64
  AES-NI, AArch64 ARMv8 crypto extension; detected at runtime through
  `core::arch`) and otherwise on plain table-driven software, which is not
  constant-time against a local cache-timing observer.

## Testing

```bash
cargo test
cargo test --release --test stepper_p6 --test stepper_r1 --test synchros --test fastrun_parity --test fastrun_speed --test flow_errors
cargo test --features=javascript_runtime --test js_commands --test js_records --test js_timeouts --test js_check --test js_jstest --test js_web --test js_modules --test js_deploy --test js_ndata --test js_refcounts --test js_reentry --test js_mcp --test js_shared
```

The suite covers JavaScript commands (with `javascript_runtime`: the result
envelope, value conversion, error reports, async commands and timers, calls
that await without holding each other up, time limits, `flowlang.call`
across languages and back into JavaScript, JavaScript tools over a real
`flowmcp` session with stdout kept to JSON-RPC, records under both language tags, `check` with and
without tsc, `flow jstest` (test files that import, and TypeScript ones, included), `fetch` against a local server and the other web
APIs, and imports — CommonJS and ESM packages, source-mapped errors,
resolution errors — TypeScript commands, and commands importing each other, through esbuild), the builder's wrapper guarantees, `Code::execute` against the stepper
(`tests/fastrun_parity.rs`: same results, errors and step counts on the
testflow commands and on generated flows; `FASTRUN_PARITY_SEEDS=N` widens the
sweep), synchro ordering under
`Code::execute` (`tests/synchros.rs`: each fixture's result differs when
`syncs` is ignored), `hex_encode`/`hex_decode`
compatibility with `encodeURIComponent`/`decodeURIComponent` (expected values
generated with Node), and RFC 7748 conformance for the vendored X25519 (both
section 5.2 vectors, the iterated test, and the section 6.1 Diffie-Hellman
vectors — expected values derived with an independent OpenSSL-backed
implementation), RFC 7693 conformance for the vendored BLAKE2b (the
Appendix A vector plus block-boundary, streaming, keyed, salt and
personalization cases — expected values derived with python's `hashlib`,
which wraps the BLAKE2 reference C code), and FIPS 197 conformance for the
vendored AES-256 (the Appendix C.3 block and A.3 key schedule, the NIST
SP 800-38A ECB vectors, and further cases derived with OpenSSL, every one run
on both the hardware and the software backend, plus a randomized
hardware-vs-software agreement sweep). The AArch64 backend can be exercised
from an x86-64 host under user-mode QEMU, which emulates the crypto
extension:

```bash
rustup target add aarch64-unknown-linux-gnu
apt-get install qemu-user gcc-aarch64-linux-gnu libc6-dev-arm64-cross
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER="qemu-aarch64 -L /usr/aarch64-linux-gnu" \
cargo test --target aarch64-unknown-linux-gnu --lib --test aes -- --skip dynlib_generations_never_unload
```

(the skipped test builds and loads a dynamic library at run time, which
user-mode emulation cannot do.)

## Best practices

- Run `DataStore::gc()` periodically in long-running services.
- Do not wrap ndata types in `Arc`/`Mutex` — they are already thread-safe.
- Use `DataStore::globals()` for state that persists across flow invocations.
- Log with `eprintln!`, never `println!`, in anything that might run under
  `flowmcp` — stdout is the JSON-RPC channel.
- For performance-critical logic, prefer one native Rust command over a large
  graph of many small operations, and mind FFI overhead when crossing language
  boundaries in a tight loop.
- Set `RUST_BACKTRACE=1` while debugging for backtraces on panics.

## Extending

New flow libraries are added by dropping their JSON definitions into the
`data/` directory — no recompilation is needed for Flow-language commands.
Rust and Python commands are added through `flowb`, which regenerates their
wrappers from the store. The architecture is designed for embedding: Newbound
uses Flowlang as the execution engine of a peer-to-peer web platform, with
commands hot-loaded from dynamic libraries.
