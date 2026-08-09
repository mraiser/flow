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

> **Note:** Support for back-end commands written in Java and JavaScript is
> kind of broken for now. Contact me if you'd like to help fix it.

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
Rust, or Python command directly.

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
stdin and the result printed to stdout as JSON.

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

**FFI isolation (advanced):** a sub-crate can be compiled as a dynamic shared
library by setting `"ffi": true` in the `cargo` section of the library's
`meta.json`. Because each shared library links its own dependencies, two
sub-projects can then depend on conflicting versions of the same crate. The
main binary needs a `build.rs` that adds the deps directory to the linker
search path:

```rust
// /my_project/build.rs
use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let profile = env::var("PROFILE").unwrap();
    let deps_path = manifest_dir.join("target").join(profile).join("deps");
    println!("cargo:rustc-link-search=native={}", deps_path.display());
}
```

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
cargo build --features python_runtime      # embeds Python 3 via PyO3
cargo build --features javascript_runtime  # embeds V8 via deno_core
cargo build --features java_runtime        # loads a JVM via JNI
```

- **Python** — commands run in a persistent embedded interpreter. Arguments
  arrive as `NDataObject` / `NDataArray` / `NDataBytes`, thin Python wrappers
  around the underlying Rust data — no serialization on the boundary, and
  Python mutates the same data Rust sees. Returning a plain value (string,
  int, dict, …) wraps it into a `DataObject` automatically. If a `venv`
  directory exists next to `data/`, it is activated automatically.
- **JavaScript** — commands run in a Deno/V8 isolate, with `serde_v8`
  bridging `DataObject` to V8 values.
- **Java** — requires the Java helper classes on the classpath and `libjvm`
  on the library path (e.g. `LD_LIBRARY_PATH`).
- **Rust** — commands are not FFI at all: `flowb` generates a wrapper and
  registry entry, and they compile into your binary (or sub-crate dylib).

## Design notes

- **Interpreter:** the heart of Flowlang is a two-phase event loop. An
  *operation pass* fires every operation whose inputs are satisfied; a
  *connection pass* propagates outputs along connections. The passes repeat
  until the graph is quiescent — a topological sort performed on the fly. A
  single flow executes on one thread, but ndata's thread-safety means separate
  `Command::execute` calls can safely run concurrently on different threads.
- **Robustness:** generated Rust command wrappers validate declared parameters
  before use and run the command body inside a panic guard. A missing or
  mistyped argument produces a structured error response — it cannot take down
  the process, which matters when commands are compiled into hot-loaded
  dynamic libraries where an escaped panic would abort.
- **Control flow:** the interpreter propagates Flow-level events ("fail",
  "terminate", "next case") as a `CodeException` enum through `Result`,
  keeping the core loop free of deeply nested conditionals.
- **Code generation over macros:** `flowb` writes ordinary Rust source
  (command wrappers, registries, the typed API) rather than using procedural
  macros — what runs is readable code checked into your tree.
- **Vendored crypto:** peer-to-peer session encryption uses a vendored X25519
  implementation (no external crypto dependency chain). It is verified against
  the RFC 7748 test vectors in this crate's test suite.

## Testing

```bash
cargo test
```

The suite covers the builder's wrapper guarantees, `hex_encode`/`hex_decode`
compatibility with `encodeURIComponent`/`decodeURIComponent` (expected values
generated with Node), and RFC 7748 conformance for the vendored X25519 (both
section 5.2 vectors, the iterated test, and the section 6.1 Diffie-Hellman
vectors — expected values derived with an independent OpenSSL-backed
implementation).

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
