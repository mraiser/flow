# **Flowlang**

## **Purpose and Core Functionality**

**Flowlang** is a Rust implementation of the **Flow language**, a dataflow-oriented programming language designed for visual "flow" diagrams. The crate's primary purpose is to **execute Flow programs** (defined in JSON) and provide a unified function-call interface across multiple programming languages, including Rust, Python, JavaScript, and Java. In essence, Flowlang acts as an **interpreter and runtime** for Flow programs, allowing developers to construct programs as dataflow graphs and run them seamlessly. This addresses the problem of orchestrating complex logic in a visual, data-driven manner, and integrating code written in different languages into one workflow.

Its multi-language support and inherent dataflow paradigm make Flowlang particularly well-suited for **building and orchestrating Large Language Model (LLM) based tools and agents**. Developers can seamlessly integrate Python scripts for LLM interactions, Rust for performance-critical tasks, and JavaScript for other utilities, all within a unified visual workflow. The flowmcp binary further enhances this by providing direct support for the Model Control Protocol.

A Flow program is represented as a directed graph of operations ("commands") where data flows along connections between nodes. The Flow language is loosely based on Prograph, a 3D visual dataflow language. Using an IDE like **Newbound**, a developer draws a diagram of how data moves through functions and conditions; Flowlang then executes this diagram by passing data through the graph. Each node (operation) processes inputs and produces outputs that feed into other nodes. The Flowlang crate essentially interprets the JSON representation of such a diagram, allowing it to run as a program.

One of Flowlang's distinctive features is **multi-language support**. It provides a unified functional API so that "Flow commands" (nodes in the flow graph) can be implemented not only in Flow's own visual language but also in **Rust, Python, JavaScript, or Java**. This means developers can write certain nodes as native Rust functions, or as Python/JS scripts, etc., and integrate them into the dataflow. The Flowlang runtime handles calling out to the correct language runtime and feeding data in/out, which simplifies building heterogeneous systems. All these languages maintain state between calls, so for example the Python interpreter or JavaScript engine isn't re-initialized on every use, enabling persistent stateful behavior across multiple calls.

**Relation to ndata:** The Flowlang crate is built on top of the companion crate ndata, which provides the dynamic data structures used to represent and pass data between flow nodes. ndata defines types like Data, DataObject, and DataArray that behave similarly to loosely-typed JSON values or Python objects. These can hold numbers, strings, booleans, nested objects/arrays, etc., and are used as the universal data container in Flowlang. Crucially, ndata implements an **internal heap with reference counting and garbage collection**. This allows Flowlang to create and pass around dynamic data (e.g., the input and output parameters to commands) without worrying about Rust's strict ownership rules—much like a garbage-collected language. In practice, every input or output in a flow is a DataObject (a JSON-like map of keys to Data values) that can be freely shared across threads and languages. The Flowlang runtime leverages ndata so that data flows smoothly through the graph, regardless of which language produced or consumes it. This design choice makes Flowlang **thread-safe by design** as ndata's objects use internal reference counts and locks so they can be sent between threads without explicit Arc wrappers. In summary, Flowlang's core functionality is enabling dataflow programming (especially visual programming via Newbound) and seamless multi-language function integration, built atop a dynamic data model provided by ndata. This empowers rapid prototyping and cross-language development by abstracting away memory management and language interop complexities.

## **Flowlang as a Premier Platform for LLM Tooling and Model Control Protocol (MCP)**

With the rise of Large Language Models (LLMs), the need for robust and flexible tooling to orchestrate LLM interactions, chain prompts, manage state, and integrate with various APIs has become paramount. Flowlang, with its inherent strengths, is exceptionally positioned as the **best vehicle for rolling your own LLM tools and agents**, especially with the introduction of Model Control Protocol (MCP) support via the flowmcp binary.

**What is Model Control Protocol (MCP)?** MCP provides a standardized way for applications to communicate with and control AI models or agents. It involves sending structured requests (often JSON-RPC) to a model endpoint and receiving structured responses. This allows for complex interactions beyond simple prompt-response, including managing context, controlling model parameters, and invoking specific agent capabilities.

**Introducing flowmcp:** The flowmcp binary in Flowlang is a dedicated executable that implements an MCP server. It listens for JSON-RPC messages over stdin, processes them using the Flowlang engine, and sends responses back via stdout. This allows external systems or interfaces to interact with Flowlang-defined workflows as if they were language models or intelligent agents.

**Why Flowlang is Ideal for LLM Tooling:**

1. **Seamless Multi-Language Integration:**


 * **Python Dominance in LLMs:** The majority of LLM SDKs (e.g., OpenAI, Hugging Face Transformers, LangChain, LlamaIndex) are Python-based. Flowlang's first-class Python support (via PyO3) allows direct embedding of Python scripts as nodes in a flow. This means leveraging existing LLM libraries and custom Python code for model interaction, prompt templating, and data processing without complex FFI wrappers.


 * **Rust for Performance:** For pre-processing, post-processing, or any performance-critical logic in an LLM pipeline, native Rust commands can be written.


 * **JavaScript & Java:** Integration with web APIs or existing Java/.js libraries is also supported.

2. **Visual Dataflow Programming for Complex Chains:**


 * LLM applications often involve complex chains of operations: fetching data, constructing prompts, calling an LLM, parsing the response, making decisions, calling another LLM or tool, and so on.


 * Representing these chains as visual Flow diagrams (e.g., in Newbound) makes them significantly easier to design, understand, debug, and modify compared to monolithic scripts.


 * The dataflow paradigm naturally maps to how data (prompts, responses, context) moves through an LLM agent.

3. **Flexible Data Handling with ndata:**


 * LLM inputs and outputs can be complex JSON structures. ndata's DataObject provides a flexible, JSON-like way to handle this data dynamically across different language components in a flow.

4. **State Management:**


 * Flowlang's ability to maintain state within language runtimes (e.g., Python interpreter, JS engine) between calls is crucial for LLM applications that require conversational memory or persistent context.


 * Global variables within Flowlang can also be used to manage shared state across different parts of an LLM agent's logic.

5. **Rapid Prototyping and Iteration:**


 * The visual nature and multi-language support accelerate the prototyping of LLM tools. Different models, prompt strategies, or processing logic can be quickly swapped by modifying the flow graph or the underlying scripts.

6. **Exposing LLM Tools as Services:**


 * With flowmcp, sophisticated Flowlang-orchestrated LLM agents can be exposed over a standardized JSON-RPC interface.


 * Additionally, Flowlang's built-in HTTP server allows easy conversion of LLM flows into web services.

**Example Use Case: A Research Agent Flow** Imagine an LLM agent that takes a research query, searches the web, summarizes relevant articles, and generates a report. In Flowlang:

* An initial node (Python) uses a search engine API.

* Multiple parallel nodes (Python) call an LLM to summarize each article.

* A subsequent node (Python or Rust) synthesizes these summaries.

* A final node (Python) uses an LLM to generate the final report based on the synthesis.

* flowmcp allows an external application to invoke this entire research agent with a single JSON-RPC call.

By leveraging Flowlang and flowmcp, developers can build powerful, modular, and maintainable LLM-powered applications with greater ease and clarity than traditional scripting approaches.

## **Key Technologies and Design (Rust Features & Concurrency)**

Despite being implemented in Rust, Flowlang adopts many techniques more common in dynamic or functional languages. Key Rust technologies and design choices include:

* **Dynamic Data with Manual GC:** Flowlang uses the ndata crate to manage data dynamically. ndata internally uses a global heap and manual garbage collection—unusual for Rust, but deliberate here to allow more flexibility. All DataObject and DataArray instances carry their own reference counts, and memory is only freed when a GC function is explicitly called. This means Flowlang can store cyclic or cross-scope data (e.g., global state or interconnected node outputs) without immediate ownership issues. The trade-off is that the programmer (or the runtime) must periodically invoke DataStore::gc() (which calls NData::gc()) to clean up unused values. This design restores some of the "garbage-collected language" convenience inside Rust's safe environment, at the cost of forgoing Rust's usual compile-time memory guarantees. It's a conscious choice to make Flowlang suitable for **rapid prototyping** and multi-language interop. In practice, when writing Rust code that uses Flowlang, **do not wrap Flow data in additional Arc or Mutex**—ndata already handles thread-safe reference counting internally. A common mistake is to put Data or DataObject inside an Arc; this is unnecessary and could lead to memory never being freed (as ndata's GC would not see the data as collectable). Instead, rely on Flowlang/ndata's own memory model and simply call the GC when appropriate (for example, after a batch of flow executions, call DataStore::gc() to reclaim heap storage).

* **Thread-Safety and Concurrency Model:** Flowlang's concurrency model is built around the idea that flows can run in parallel, but individual flow executions are single-threaded by default. The Flow interpreter uses an event-loop style algorithm to evaluate the dataflow graph (detailed in the next section) and does not spawn multiple threads for parallel nodes—instead, it processes nodes whose inputs are ready in sequence. However, because ndata data structures are thread-safe, it is possible to run multiple Flow **commands (functions)** concurrently in different threads or tasks. For example, two separate Command::execute calls can happen on different threads—the underlying data passing (using DataObject) is protected by atomic reference counts and locks, so data races will not occur. In short, Flowlang itself doesn't automatically parallelize a single flow, but it *allows multi-threaded use*. The thread safety is achieved without heavy use of Mutex thanks to the internal design of ndata: references to data are coordinated by a custom thread-safe reference counter (SharedMutex in ndata) so that cloning a DataObject just bumps a count and different threads can read/write through it safely. This simplifies concurrent scenarios—manual copying or guarding of flow inputs/outputs to share them is not needed. The Flowlang interpreter loop also uses only safe Rust (no unsafe for concurrency), leaning on the atomic refcounts for synchronization. There is no explicit use of Rust async/await in Flowlang; flows are generally run to completion synchronously via Command::execute. If asynchronous behavior is needed (e.g., waiting on I/O), implement that inside a node (for instance, a Rust node can use tokio internally, or a JavaScript node can await a promise in the embedded engine).

* **FFI and Language Embedding:** Under the hood, Flowlang leverages **Rust's FFI capabilities** to integrate other language runtimes:


* For **JavaScript**, it includes an optional feature to embed the Deno/V8 engine. The crate depends on deno_core and serde_v8; when the javascript_runtime feature is enabled, Flowlang spawns a V8 isolate (via Deno's core) to execute JS code. Each JS-based flow command is run in this engine, with data passed through JSON serialization (serde_v8 bridges Rust DataObject to V8 values).


* For **Python**, Flowlang uses pyo3 (via a python_runtime feature) to embed a Python 3 interpreter. Rust functions can call into Python, and Python-defined flow commands are executed in the same interpreter (maintaining state, e.g., global variables, between calls). The python_runtime feature auto-initializes the interpreter on startup. This is particularly crucial for LLM tooling, where Python is prevalent.


* For **Java**, Flowlang employs the Java Native Interface (JNI) (jni crate) when java_runtime is enabled. Java support is the most involved, requiring specific Java helper classes (e.g., Startup.java and associated packages) to be present in the classpath. If configured, Flowlang loads the JVM (requiring libjvm.so or its equivalent to be on the system's library path, e.g., LD_LIBRARY_PATH) and can call Java methods for flow commands. Each language integration runs in-process with Flowlang, so data conversion and calls happen via FFI (for Python/JS) or JNI (for Java).


* For **Rust (native) functions**, Flowlang has a special mechanism. Rather than FFI, Rust commands are compiled into the project and registered. The Flowlang crate includes a separate binary called flowb ("flow builder"), which generates Rust source stubs for any Flow commands meant to be implemented in Rust and compiles them into the project. Essentially, a Rust function is written for a flow node, flowb is run to integrate it, and then Flowlang can call it directly as part of the flow. Internally this is handled by a module that registers Rust command pointers. For example, after generating Rust commands, an initialization function is called (done via an internal cmdinit routine) to register all new Rust commands with the runtime. These Rust commands are then invoked directly when their node executes, which is efficient (no FFI needed since it's within the same binary).

All these integrations highlight Rust's ability to host multiple runtimes simultaneously. Flowlang uses conditional compilation (feature flags) to keep these optional—by default, only pure Flow and Rust are supported, and one compiles with --features=javascript_runtime or others to include JS, Python, or Java support. This modular design keeps the base crate lightweight and lets users opt-in only to the needed language engines.

* **Macros and Code Generation:** The Flowlang codebase itself doesn't rely heavily on procedural macros, but it does generate code at build-time for Rust commands. When flowb is run, it programmatically writes out a Rust source file containing stubs to call user-defined Rust functions and a registry of those functions. This file is included via mod cmdinit in the crate. At runtime, the crate calls a function (generated in that module) to register these commands. For example, after initialization, Flowlang calls an internal cmdinit() which populates a list of command metadata, then calls RustCmd::add(...) for each, effectively telling the interpreter "if command X is called, run this Rust function". This approach uses Rust's compile-time generation rather than a macro, but the effect is similar to a plugin system. There are also some uses of attributes like #[cfg_attr(feature="serde_support", ...)] in the data structures (e.g., auto-deriving Serialize/Deserialize for the flow graph structs when serialization is enabled). These conditional derives make it easy to dump or load flow definitions via serde_json when needed (mostly for debugging or storage).

* **Error Handling and Control Flow:** The interpreter uses Rust Result and a custom CodeException enum for internal control flow. For example, if a node signals a failure or a termination, the interpreter returns a CodeException::Fail or ::Terminate which unwinds the execution loop in a controlled way. This is how Flow-level control structures like "stop flow" or "goto next case" are implemented. Rust's match and error handling are used here instead of exceptions; but conceptually, they serve a similar role to propagate events like "skip to next branch" up to the main loop. This design keeps the core loop clean and avoids deeply nested conditionals. Also, any Rust panic inside a Rust-based command will not automatically crash the Flow runtime; since ndata retains data on panic, an unwind could be caught if necessary. Panics propagate unless caught by the embedding application. Flowlang encourages signaling errors via the Fail exception path rather than panicking.

In summary, Flowlang's architecture is an interesting blend: it sacrifices some of Rust's usual strictness (using a global heap and dynamic typing) to gain flexibility, while still leveraging Rust's strengths in FFI, speed, and safety for multi-language support. The concurrency model is cooperative and data-driven—multiple languages run in the same event loop and thread, unless they are explicitly threaded out. The design emphasizes that data is the primary carrier of state (fitting a dataflow paradigm), and everything from memory management to multi-language calls is built to make passing around DataObject instances simple and safe.

## **Installation and Usage**

Flowlang can be used both as a **standalone binary** and as a **library** crate in a Rust project. Depending on the use case, installation can be done either way:

* **As a Binary (CLI Tool):** The crate comes with three binaries: flow (the main interpreter), flowb (the builder for Rust/Python commands), and flowmcp (for Model Control Protocol interactions). Obtain these by cloning the GitHub repo and building:

  ```
  git clone https://github.com/mraiser/flow.git
  cd flow
  cargo build # builds the flow, flowb, and flowmcp binaries

  # (Optionally, copy or symlink the binaries to a directory in your PATH)
  sudo ln -s $(pwd)/target/debug/flow /usr/local/bin/flow
  sudo ln -s $(pwd)/target/debug/flowb /usr/local/bin/flowb
  sudo ln -s $(pwd)/target/debug/flowmcp /usr/local/bin/flowmcp
  ```

  This compiles the latest code. (For a release build, use cargo build --release and adjust the paths accordingly.) Once built, the flow CLI can execute Flow libraries. By default, it looks for a data directory in the current working directory which contains the flow libraries (JSON files). The repository itself includes a data/ folder with an example library called **"testflow"**.

* **To** run **a flow from the command line with flow**, use:

  ```
  flow <library> <control> <command> <<< '<json-input>'
  ```
  
  For example, to execute the test_add command in the testflow library:
  
  ```
  cd path/to/flow # directory containing 'data' folder
  flow testflow testflow test_add <<< '{"a": 300, "b": 120}'
  ```

  This launches the Flow interpreter, loads the **testflow** library, and runs the function named **test_add** with the provided JSON input (here a=300, b=120). The result is printed to stdout as JSON. In this case, test_add adds the two numbers and would output {"result": 420} (for example). The general format is flow <lib> <ctl> <cmd>, where <ctl> is a category or control within the library—often this is the same as the library name if subcategories are not used. There are also special built-in controls: e.g., `flow flowlang http_listen` can start an HTTP server.

* **To use flowmcp for Model Control Protocol interactions:** The flowmcp binary starts a server that listens for JSON-RPC requests on stdin and sends responses to stdout. This is designed for programmatic interaction, where another process sends requests to flowmcp to execute flows.

  ```
  # Run flowmcp (it will wait for JSON-RPC requests on stdin)
  ./target/debug/flowmcp
  ```

  An external application would then pipe JSON-RPC requests like the following to flowmcp's stdin:

  ```
  {"jsonrpc": "2.0", "method": "testflow.testflow.test_add", "params": {"a": 5, "b": 7}, "id": 1}
  ```

  And flowmcp would respond on stdout:

  ```
  {"jsonrpc":"2.0","result":{"result":12},"id":1}
  ```

  The `gag` feature is useful with flowmcp to suppress stdout from underlying flow commands, ensuring only JSON-RPC responses are emitted.

* **As a Library in Rust:** Include Flowlang in a Cargo project by adding to **Cargo.toml**:

  ```
  [dependencies]
  flowlang = "0.3.18" # Or the latest version
  ndata = "0.3.13" # Or the version compatible with your flowlang
  ```

  Make sure to use the latest version from crates.io. With this, the Flow runtime can be initialized and flows executed from Rust code. A minimal example to run the same test_add function:

  ```
  use flowlang::datastore::DataStore;
  use flowlang::command::Command;
  use ndata::dataobject::DataObject;
  use flowlang::init; // For flowlang::init

  fn main() {
      // Initialize the Flow runtime with the path to the data libraries:
      init("data"); // Recommended: sets up DataStore and registers Rust commands
      std::env::set_var("RUST_BACKTRACE", "1"); // for debugging, if needed

      // Prepare input as a DataObject (from JSON string):
      let args_json = r#"{"a": 299, "b": 121}"#;
      let args = DataObject::try_from_string(args_json).expect("Failed to parse JSON input");

      // Lookup the command by library, category, and name:
      let cmd = Command::lookup("testflow", "testflow", "test_add").expect("Command not found");
      // Execute the command:
      match cmd.execute(args) {
          Ok(result) => {
              println!("Result = {}", result.to_json_string().unwrap_or_else(|_| "Error converting to JSON".to_string()));
          }
          Err(e) => {
              eprintln!("Flow execution error: {:?}", e);
          }
      }
      DataStore::gc(); // optional: run garbage collection
  }
  ```

  In this snippet, the Flow environment is initialized by calling init("data"). This loads the libraries from the data directory and registers any compiled Rust commands. A DataObject is then constructed from a JSON string for the input arguments. Command::lookup(lib, ctl, name) retrieves a handle to the specified Flow command. Finally, cmd.execute(args) runs the flow and returns a Result<DataObject, CodeException>. The output DataObject can be converted to standard JSON via to_json_string() for printing. The example sets an env var RUST_BACKTRACE because Flowlang captures errors in Rust commands and a backtrace may be desired if something fails inside a Rust node. After use, DataStore::gc() can be called to clean up any leftover dynamic data. (In long-running processes, GC might be called periodically or at program end.)


**Integration with ndata:** ndata::DataObject will be primarily used for constructing inputs and reading outputs. DataObject behaves much like a serde_json::Value (specifically, an object map). try_from_string can be used as shown to parse a JSON string into a DataObject, or one can be built programmatically by inserting keys. Data values (which can be ints, floats, etc.) can also be directly manipulated, but treating it as JSON is often simplest. Keep in mind that these objects live in ndata's heap; if a DataObject is cloned, it will increase a refcount, not deep-copy the data. To extract primitive Rust values, methods like DataObject::get_int("field") can be used, or it can be converted to a serde_json::Value via to_json if the serde_support feature is enabled.

* **HTTP Service Usage:** Flowlang has a built-in mini HTTP server that can expose flow commands as web endpoints. This is invoked via the CLI. For example:
  ```
  flow flowlang http listen <<< '{"socket_address": "127.0.0.1:7878",
                                    "library": "flowlang",
                                    "control": "http",
                                    "command": "parse_request"}'
  ```

  This command starts an HTTP listener on port 7878. Now, any Flow command can be invoked by an HTTP GET. For instance, after starting the server, visiting: [http://127.0.0.1:7878/testflow/testflow/test_add?a=42&b=378](http://127.0.0.1:7878/testflow/testflow/test_add?a=42&b=378) would trigger the test_add command in testflow with {"a":42,"b":378} as input, and the result would be an HTTP response. This feature is very useful for quickly turning flow libraries (including LLM-powered ones) into web services or microservices. The example above uses flowlang's internal http.parse_request handler to translate HTTP queries to Flow inputs.

* **Enabling Language Runtimes:** If flows include commands written in JavaScript, Python, etc., compilation must include the corresponding features:


* **JavaScript:** cargo run --features "javascript_runtime" --bin flow ... will enable the Deno/V8 integration.


* **Python:** cargo run --features "python_runtime" ... enables Python—after ensuring Python3 is installed. flowb all generates any needed Python stubs before executing Python nodes. The flowb tool automatically extracts embedded Python code from the flow definitions and writes .py files (and similarly for Rust .rs files). This is essential for LLM workflows.


* **Rust:** No feature flag needed (Rust support is always compiled in), but flowb must be run to generate and compile the Rust code for custom Rust-implemented flow nodes. For example:

  ```
  ./target/debug/flowb ALL # rebuild all Rust/Python commands for all libraries
  ./target/debug/flowb testflow testflow test_rust # build only test_rust command
  ./target/debug/flow testflow testflow test_rust <<< '{"a":"world"}'
  ```

  The first line rebuilds all, the second explicitly rebuilds one command, and the third executes it.


* **Java:** Compile with --features "java_runtime", and add the required Java files (as specified in Flowlang's documentation or examples) into the project. Specific Java directories (e.g., historically botmanager, peerbot) and .java files must be placed appropriately, Startup.java compiled, and the JVM library available. This setup is more complex, but it essentially boots a JVM inside Flowlang so that any flow command marked as a Java command will call into the configured Java code environment.

Overall, installing and using Flowlang requires a bit more setup when multiple languages are involved, but the crate is flexible. An IDE like Newbound automates these steps (Newbound calls flowb, manages codegen, etc.). When using Flowlang programmatically, initialize with init("data") before execution. Once that's done, using Command::execute is straightforward. The crate also provides lower-level APIs if needed (for example, raw JSON for a library can be fetched or existing commands queried via DataStore::lib_info), but the primary usage pattern is as shown.

## **Code Structure and Flow Execution Architecture**

Internally, Flowlang represents a flow (i.e., a function in the Flow language) as a collection of interconnected components. Understanding the crate's structure clarifies how Flow programs are defined, parsed, and executed:

* **Modules Organization:** The crate is divided into several modules, each handling a portion of the functionality:


* datastore: Manages loading and storing of flow definitions (the JSON files) and provides global storage for runtime data.


* command: Defines the Command struct and lookup/execute logic for commands.


* code and case: These are core to the interpreter. The case module defines the in-memory structures for a flow's logic (like nodes and connections), while code contains the Code struct and the algorithm to run a flow.


* primitives: Contains basic built-in operations (e.g., arithmetic, comparisons) that the interpreter can execute directly.


* rustcmd, pycmd, jscmd, javacmd: These handle the integration for each external language. For example, rustcmd::RustCmd struct and methods to register and call Rust-based commands, pycmd for executing Python code, etc.


* mcp: Contains the logic for the Model Control Protocol server, primarily used by the flowmcp binary.


* buildrust: Functions to generate Rust code (build_all, build_lib, etc.) for the flowb tool.


* Utility modules like base64, rand, sha1, rfc2822date implement certain primitives or library functions in Rust (e.g., for randomness or encoding).


* appserver: Implements the HTTP server logic.

The crate's lib.rs aggregates these modules. Notably, there is a private module cmdinit which contains the auto-generated code for Rust commands (this is empty by default or filled by flowb).

* **Flow Definition Data Structures:** When a flow library (JSON) is loaded, it is parsed into a set of in-memory structs:


* **Case** (flowlang::case::Case): This struct represents a flow function's code—analogous to a function body or a code block. The name "Case" comes from Flow's heritage (it can represent a branch or case in logic). A Case contains:



* input and output: HashMaps of String -> Node, defining the named input parameters and output parameters for this flow.



* cmds: a Vec<Operation>—the list of operations (nodes) in this flow.



* cons: a Vec<Connection>—the list of connections (edges) linking outputs to inputs.



* nextcase: an Option<Box<Case>>—this allows a flow to link to another Case, enabling multi-phase execution or branching. For example, an **if-else** is represented as two Cases where one's nextcase points to the alternative branch's Case. A loop has a Case for the loop body and uses nextcase to indicate the next iteration.


* **Operation** (flowlang::case::Operation): Represents a single operation/node in the flow graph. Key fields include:



* input / output: HashMaps of named inputs and outputs for that node (each a Node). For instance, a node that adds two numbers has inputs "a", "b" and output "sum".



* cmd_type: A string indicating what kind of operation this is. This includes "rust", "python", "flow", "if", etc., depending on how the JSON defines it. (In the JSON, this is the "type" field for the node).



* ctype and cmd: Optional strings for further specifying the command to call. For example, if cmd_type is "flow" (meaning this operation calls another flow function), ctype holds the target library name and cmd the function name. Or if cmd_type is "rust", ctype is an identifier for which Rust function to call (the crate uses these to look up the function pointer).



* name: A name/ID for the operation (often an auto-generated unique name or a user-friendly label).



* pos and width: These are primarily for visual IDEs (3D position and display width of the node in the visual editor). They don't affect execution except for potential ordering.



* localdata: An optional Case boxed inside—this is used if the operation has its own sub-flow defined within it. This can happen for things like loops or user-defined Flow commands: for example, a **loop node** carries a localdata which is the Case for the loop body.



* condition: An optional Condition struct, used if this operation has a condition (for instance, an if node has a condition with a rule and boolean value).



* result: An Option<DataObject> to hold the execution result once the node runs.



* done and finish: Booleans tracking if the operation has executed (done) and if it is a terminating node (finish). A node with finish=true indicates the flow should terminate after this (e.g., a "Return" node).


An example test_add command in JSON is parsed into a Case with two input Nodes ("a", "b"), one output Node ("result"), and one Operation for the addition. That Operation has cmd_type = "primitive", cmd = "add" (if using built-in addition logic), or it could be cmd_type = "rust" and refer to a Rust function that adds. The connection list (cons) links the Case's input nodes to the Operation's inputs, and the Operation's output to the Case's output node.


* **Node** (flowlang::case::Node): Represents a data node, typically a placeholder for a value either waiting to be produced or already produced. A Node has:



* mode and cmd_type: Strings describing the node type. For example, mode distinguishes between a literal constant vs. a variable input. cmd_type here mirrors the data type or usage (this includes "int", "string", or things like "list" if the node represents a collection).



* val: Data: the actual data value if it's been set (initially DNull for not set).



* done: bool: flag indicating if the value is available (true when the value has been computed or provided).



* list and looop: Options used for complex structures (e.g., list holds an identifier if the node is part of a list, looop is notably spelled with three "o" to avoid the Rust keyword, representing loop-related information). These are mainly used in advanced flow structures like loops (for example, marking a node as the loop index or loop condition).


The input/output maps in Case and Operation use Node to represent the "ports" of a function or operation. Initially, all input Nodes for an Operation are not done (no value). As connections feed them, their val gets set and done becomes true. Similarly, an output Node will get its val when the operation executes.


* **Connection** (flowlang::case::Connection): Represents a directed link from a source to a destination. It has:



* src: Dest and dest: Dest. Dest is a small struct with an index (i64) and a name (String). Here, index refers to an operation index or a special code, and name is the name of the Node at that index.



* done: bool: indicating if this connection has finished transferring its value.



The special values for Dest.index are important: Flowlang uses -1 to denote the **Case's input** space, and -2 to denote the **Case's output** space. In other words:



* If a Connection's src.index == -1, the source is one of the function's inputs. The src.name then corresponds to a key in the Case's input map. (E.g., src = (-1, "a") means take the value from the function's input a.)



* If a Connection's dest.index == -2, the destination is a function's output. The dest.name is a key in the Case's output map. (E.g., dest = (-2, "result") means this connection delivers into the function's output result.)



* Otherwise, a positive or zero index refers to an index in the Case.cmds vector (i.e., a specific Operation in this Case). For example, src.index = 3, src.name = "out1" would refer to the Operation at index 3 in the list, specifically the Node named "out1" in that operation's output map.

Connections essentially form the edges of the graph, connecting outputs of one operation (or inputs of the whole flow) to inputs of another operation (or outputs of the whole flow). When the JSON is parsed, each connection is built via Connection::from_data, which reads something like {"src":[<index>,"<name>"], "dest":[<index>,"<name>"]} to populate the Dest structs.

* **Loading and Parsing Flows:** When init("data") (or DataStore::init("data")) is called, the crate reads the JSON files in the data directory and constructs these structures. Typically, each library is a folder under data (e.g., data/testflow/) containing JSON files for each command or a single JSON for the library. Libraries created by Newbound are placed in data/ and become executable when present. The code expects a structure like <library>/<control>/<command>.json or similar. The specifics can vary, but DataStore::get_data_file and read_file functions handle retrieving the JSON text. The JSON is then parsed (using serde_json if the serde_support feature is enabled, or via ndata::json_util) into a DataObject, and then into a Case with Case::from_data(data_object). The from_data implementation iterates through each key ("input", "output", "cmds", "cons", "nextcase") to build the corresponding Rust structures. After parsing, the library's commands are stored, likely in a global map keyed by library/control/command name. Each command is assigned an ID (the Command.id field) and has a Command struct created.


The Command struct (in flowlang::command::Command) holds metadata to identify and invoke a flow command. Its fields include:


* lib (library name), name (command name), and possibly lang or cmd_type (indicating if it's a Flow-implemented command or a native one).


* params, readers, writers, return_type describing the input/output types or access patterns (these might not be fully utilized in the current version, but reserved for describing data access patterns).


* src stores a reference to the parsed Case or to the native function backing this command.

When Command::lookup("testflow","testflow","test_add") is called, the crate searches in its registry (using DataStore::lookup_cmd_id or similar) and returns a Command instance if found. The Command::execute(args) method then delegates to the proper execution path: if the command is a Flow-defined function, it invokes the interpreter on its Case; if it's a Rust/Python/JS command, it calls the corresponding native function or script.

* **Interpreter Execution Algorithm:** The heart of Flowlang is the interpreter that executes a Case. This lives in flowlang::code::Code::execute. When a Flow command is executed, Flowlang creates a Code object (which contains a Case and some flags) and calls Code.execute() with the input arguments. The execution loop proceeds as follows:


* **Initialization:** The input DataObject (containing the arguments) is available. The Case for this code is duplicated (cloned) into a current_case so that modifications (like marking nodes done) don't alter the original definition. The out DataObject is created to collect outputs. A flag done = false indicates the flow is still running.


* **Main Loop:** While not done, the interpreter performs two passes: one over operations and one over connections:



* **Operation pass:** Iterate through each Operation in current_case.cmds. For each:




* If the operation is not yet done, check its inputs. For each input Node in op.input, determine if it has an incoming connection that hasn't delivered a value yet. This is done via lookup_con(cons, key, "in") which searches the cons list for a connection whose dest name matches this input name. If a connection is found and it's not completed, this input is waiting on another operation's output—so this operation cannot be executed yet (a flag indicates inputs not ready). If no connection is found for an input, it means the input is either a constant or already has a value; the code marks that input Node as done (since its value is essentially immediate or was provided).




* After checking all inputs, if either there were no inputs (count == 0) or all inputs are ready, then the operation can fire. The interpreter calls self.evaluate(cmd) to execute the node's logic. This performs the operation—for a Flow-defined subroutine, it recursively calls into another Code::execute; for a primitive or native function, it calls the appropriate function. The evaluate function prepares a DataObject of all input values (in1) and depending on cmd.cmd_type, does something like:





* If cmd_type == "flow", it finds the Command for cmd.ctype/cmd and calls that (which invokes another flow or native function).





* If cmd_type == "rust" or others, it delegates to RustCmd/PyCmd etc. to run the code.





* If cmd_type corresponds to a built-in (e.g., an arithmetic op), it is executed directly in Rust (the primitives::Primitive handles this). The result from the operation (a DataObject) is stored in cmd.result and cmd.done is set true. If the operation was marked with finish=true (like a Return), evaluate returns a CodeException::Terminate to signal the flow should stop after this node.



* **Connection pass:** After attempting to evaluate nodes, the interpreter goes through each Connection in current_case.cons. For each connection that is not yet done:




* Determine if the source is ready to transmit. If con.src.index == -1, the source is the function input. It checks if the input args contain the src.name; if yes, that value is taken as val, and a flag indicates the value is ready. If src.index != -1, then the source is an operation: the operation is retrieved and checked if it's done; if yes, the value from that operation's result[src.name] is taken as val, and a flag indicates the value is ready.




* If a value is ready, the connection is marked as done (it will not be used again). Then deliver the value to the destination:





* If dest.index == -2, this value goes to the final output. The code executes out.set_property(destname, val)—putting the value into the output object that will be returned.





* Otherwise, dest is an operation index. The operation is fetched and the input Node with name destname is found. That Node's val is set to val and it is marked done = true. Then it checks if all inputs of that dest operation are now done; if yes, it immediately calls self.evaluate(dest_op) to run that operation. This last step means as soon as an operation's all inputs become available in the middle of the connection pass, it will trigger its execution, logically ensuring no unnecessary delay in node execution.




* The connection loop continues until no incomplete connections are found. If all connections are done, done is set to true, meaning the flow has no more pending data transfers, and the main loop can be exited.



* These two passes (operations, then connections) together constitute one iteration of the main while !done loop. Notably, the algorithm executes multiple operations per iteration if their inputs become ready one after the other during the connection processing. It effectively performs a **topological sort** of the graph on the fly, executing nodes as soon as their dependencies are satisfied.


* **Exception Handling:** During execution, if any operation's evaluation throws a CodeException (as mentioned, for control flow):



* CodeException::Fail—indicates an error in a node. The interpreter stops and returns an Err (propagating the failure).



* CodeException::Terminate—indicates a normal termination (like hitting a return). The interpreter breaks out of the loop and returns the results gathered so far as Ok.



* CodeException::NextCase—if a node signaled this (e.g., a special node for "move to next case"), the interpreter switches current_case to the Case in current_case.nextcase and continues execution. This is how branching to an alternate case (like else-branch or continuing after a case block) is handled. It essentially replaces the current flow graph with another and keeps going. If nextcase was None, it is treated like a termination.


* **Completion:** Once the loop ends (either naturally or via Terminate), the function returns the out DataObject as the result. This contains whatever values were set to dest index -2 (the outputs). If no outputs were set, it is an empty object.

The execution is deterministic and single-threaded—it always iterates through nodes in order of their indices. The indices correspond to the visual layout ordering or an insertion order. This generally does not affect execution order because data dependencies govern it. But if two independent subgraphs exist, the interpreter checks them in a fixed sequence each iteration. While this could be optimized, it is acceptable for typical flow sizes. Also, by marking nodes and connections as done and by removing finished connections from consideration, the loop's workload decreases as it progresses.

* **DataStore and Command Resolution:** The DataStore keeps track of libraries and commands so that when Command::lookup is called, it finds the right Case or native function. Internally, lookup_cmd_id(lib, ctl, cmd) constructs a file path or a key and ensures that library JSON is loaded (perhaps on-demand). The DataStore also holds a globals DataObject which can be used for global variables across flows. This enables storage of state accessible by any flow (for example, if one flow sets globals().set("X", someData), another can read it). It is implemented as a static DataObject.

* **Extending Flow Behavior:** Flowlang has some additional internal features:


* The primitives::Primitive type enumerates built-in operations (such as arithmetic, string ops). If a flow node's cmd_type matches a Primitive, evaluate executes it directly in Rust for speed. For example, an add node corresponds to Primitive::Add and the interpreter performs integer addition.


* The **mirror** functionality: flowlang::mirror((dir, config)) is provided to mirror the data store in another process. This is used for hot reloading: if new code is loaded (a new version of the flow library, or recompiled Rust commands as a new dynamic library via hot-lib-reloader), the old state (heap data, etc.) can be mirrored into the new process so that execution can continue without losing information. In practice, NData::mirror and DataStore::mirror enable transferring the entire heap and global state to a reloaded instance. This is an advanced feature used, for example, in live coding environments like Newbound. It shows that internal data structures can be snapshotted and cloned across process boundaries (since they are essentially just reference-counted indices into a heap, the mirror function maps the memory into the new process).

* **Integration Points:** The modules pyenv, jscmd, javacmd each define how to initialize and execute commands in their respective languages. For example, when a Python-based command is encountered in evaluate, it calls pycmd::exec(py_code_id, args), which uses the Python C API via pyo3 to run the code and return a DataObject. Similarly, jscmd uses Deno's V8 isolate to run JS (by constructing a JS function call with the args). The results are converted back into ndata::DataObject so the rest of the flow can use them. Each language runtime is initialized once (e.g., one JS isolate, one Python interpreter, one JVM) and persists in DataStore or static variables for reuse, to maintain that state is preserved between calls (so, for example, a Python command can import a module and on the next call it's still in memory).

In summary, the code structure shows a classic dataflow interpreter: a set of structures for nodes and links, and a loop that propagates data through the graph. Flows are defined declaratively in JSON, but once loaded into Case structures, they are executed imperatively by the Rust interpreter. The architecture cleanly separates concerns: data management (ndata), flow graph definition (case/code), foreign function interfaces (rustcmd, pycmd, etc.), and model control (mcp). The design is flexible—new features like additional primitive operations or even new language integrations can be added by extending these modules. The use of JSON as the definition format means flows can be generated or edited with tools, and the interpreter doesn't need to compile them (it operates directly on the data structure). While the execution engine is not inherently parallel, it ensures predictable and ordered processing which is important for reproducibility in visual programming.

## **Examples and Best Practices**

Consider a simple example of Flowlang usage and best practices:

**Example:** Suppose a flow is needed to compute a * b + c. This can be done by writing a Rust function or assembling a Flow visually. If done in Flow for illustration: A library (e.g., mathflow) and a command mul_add would be created. The Flow's JSON would conceptually be:

```
{
  "input": { "a": {"type": "number"}, "b": {"type": "number"}, "c": {"type": "number"} },
  "output": { "result": {"type": "number"} },
  "cmds": [
    {
      "name": "multiplyNode",
      "type": "primitive", /* Or "rust", "python" if custom */
      "cmd": "multiply",   /* Name of the primitive or registered command */
      "input": { "x": {"type": "number"}, "y": {"type": "number"} },
      "output": { "product": {"type": "number"} }
    },
    {
      "name": "addNode",
      "type": "primitive",
      "cmd": "add",
      "input": { "p": {"type": "number"}, "z": {"type": "number"} },
      "output": { "sum": {"type": "number"} },
      "finish": true
    }
  ],
  "cons": [
    { "src": [-1,"a"], "dest": [0,"x"] },
    { "src": [-1,"b"], "dest": [0,"y"] },
    { "src": [0,"product"], "dest": [1,"p"] },
    { "src": [-1,"c"], "dest": [1,"z"] },
    { "src": [1,"sum"], "dest": [-2,"result"] }
  ]
}
```

This is a simplified representation (actual JSON includes full Node definitions for each input/output with modes, etc.). When executed, Flowlang takes a, b, c from inputs, sends them to the multiply node (multiplyNode), executes it to get product, sends that and c to the add node (addNode), executes it to get sum, then marks that as the final result. The "finish": true on the add node indicates it's the final operation, so the interpreter terminates after that (setting a Terminate exception). The output would be in result.

If implementing this via the Rust API:

* Place the JSON in data/mathflow/mul_add.json, call flowlang::init("data"), then use Command::lookup("mathflow","mathflow","mul_add").execute(args).

* Ensure primitive operations like "multiply" and "add" are recognized. (Flowlang's primitives include basic math; otherwise, they can be implemented as Rust commands.)

**Best Practices & Caveats:**

* **Memory Management:** Run DataStore::gc() at appropriate times (especially in long-running services). Since Flowlang does not free Data on drop automatically, failing to call GC can lead to memory bloat. A typical pattern is to call gc() after a batch of flows or when an idle period is reached. The NDataConfig returned by init can be tuned (e.g., to set heap size limits if supported by the NDataConfig version).

* **No External Sync Needed:** Do not wrap Flow data in additional synchronization primitives. DataObject and related types are thread-safe. Wrapping them in an Arc<Mutex<...>> would be redundant and could even cause logical errors (e.g., deadlocks or missed GC). If a piece of data needs to be shared between flows or threads, the DataObject reference can just be shared; cloning it simply bumps a refcount. This is one of the conveniences of Flowlang's design.

* **Global State:** For global state across flow invocations, use DataStore::globals() to get the global DataObject and store keys in it. This is preferable to having truly static globals in Rust, as it integrates with Flowlang's GC and mirror functions. For example, DataStore::globals().put_int("counter", 0); initializes a counter accessible to any flow. Keep in mind that global data will persist until explicitly removed or the DataStore is destroyed.

* **Using Multi-Language Commands (especially for LLMs):** When writing flow commands in other languages, ensure the initialization steps are followed:


* For Python (critical for LLMs), if a new flow command that runs Python code is created, run flowb all (or flowb <lib> <ctl> <cmd>) to generate the stub and the .py file. The Python file may need to be edited to implement the function logic (an IDE like Newbound can assist with this). Then, run with the python_runtime feature. Ensure the Python environment has necessary LLM SDKs installed.


* For JavaScript, enable javascript_runtime and note that Flowlang uses Deno's runtime: the JS code runs in a sandbox. If Node-like APIs or specific JS libraries are needed, functionality is limited by what Deno Core provides (which is basic ES capabilities, without Node's built-ins unless explicitly provided).


* For Rust commands, after writing the Rust function (following the template that Flowlang expects, e.g., a specific signature returning a DataObject), use flowb to integrate it. The flowb tool will insert an entry in the cmdinit module so that flowlang::init registers it. A common mistake is forgetting to re-run flowb after changing Rust command definitions—if the Rust logic is modified, the crate must be recompiled anyway, but if new commands are added, ensure they get registered.


* Flowlang's Rust command registration uses an index (u16) for the command and stores a pointer to the function. These indices must remain in sync if dynamically loading libraries. This is generally handled automatically, but care is needed if manually modifying generated code.

* **Performance Considerations:** Flowlang is optimized for flexibility, not raw speed. If performance-sensitive inner loops exist, implement that part as a native Rust command rather than as a large flow graph of many tiny operations. For example, a flow that processes a big array element-by-element in a loop incurs interpreter overhead per element; writing a custom Rust command to handle the entire array in one go is much faster. It can still be integrated via Flowlang—the flow calls that Rust command as a single node.


Also be aware of the overhead when crossing language boundaries. Calling a Python or JavaScript snippet from Flow has the cost of the interpreter. If done thousands of times, it can become a bottleneck. In such cases, batch work on the foreign language side (e.g., do more work per Python call rather than many tiny Python calls). This is especially relevant for LLM calls; each Python node should perform a meaningful unit of work with an LLM.


The internal event-loop interpreter is written in Rust and is efficient for moderate graph sizes (the operations loop and connections loop are O(N) per iteration). But if a flow graph is extremely large (say hundreds of nodes), the sequential scan can start to lag. As a best practice, structure flows hierarchically—break large flows into sub-flow commands (functions) so that each one remains manageable. This also improves clarity and reusability.

* **Hot Reloading:** When using the hot-reload capability (with the hot-lib-reloader crate and Flowlang's mirror), the data heap must be mirrored, and commands re-registered in the new library. Flowlang's mirror() function calls DataStore::mirror and then cmdinit again to re-add Rust commands. This is handled if hot-lib-reloader is integrated properly. Design flows to be idempotent or restartable so hot reloads do not leave them in an inconsistent state.

* **Debugging Flows:** Flowlang can be debugged in a visual IDE like Newbound. Outside an IDE, insert temporary logging in Rust nodes or use println!/eprintln! in the Flowlang source (some commented-out prints exist in the code, e.g., logging when an undefined command is marked done). Setting RUST_BACKTRACE=1 gives backtraces on Rust panics, which is helpful if a Rust command crashes. For debugging logic, leverage the fact that Flowlang interprets JSON—sometimes printing the parsed Case structure (via serde_json::to_string if enabled) can help understand the internal flow structure. For flowmcp, eprintln! is useful for logging as it goes to stderr and won't interfere with the JSON-RPC on stdout.

* **Common Mistakes Recap:**


* Forgetting to enable a runtime feature when needed (result: the command will not be found or will error at run-time).


* Not calling flowlang::init or using the wrong path (result: Command::lookup fails because the library isn't loaded).


* Not re-running flowb after adding or modifying the signature of new Rust commands (result: those commands may not be registered correctly or use outdated stubs).


* Wrapping DataObject in Arc (unneeded) or copying data out of DataObject unnecessarily—instead, use DataObject's methods to get what is needed.


* Neglecting to run GC, causing memory use to climb.

## **Extensions and Advanced Features**

Flowlang is not just a static interpreter—it has several advanced capabilities:

* **Plugins/Custom Libraries:** Extend Flowlang by writing libraries of Flow commands (just adding JSON in data/), or even by embedding Flowlang in another application and feeding it JSON definitions dynamically. The interpreter doesn't require files—Case::new(json_str) can parse a flow JSON from memory for execution. This means Flowlang can be used as a scripting engine: a Rust application can generate flow JSON (perhaps from a user interface or configuration) and run it on the fly. The output is a DataObject which can then be handled in Rust. This dynamic loading makes it possible to use Flowlang as a plugin system—for example, users of an app could drop in new flow definitions to add functionality without recompiling the app.

* **Hot Reloading:** Flowlang works with the hot-lib-reloader crate to allow live code updates. An IDE like Newbound uses this to compile edited Rust command code in the background and swap it without restarting the Flow runtime. The mirror function plays a role here, cloning the data heap into the new library. The state of all global and static data (including Python interpreter state, etc.) is preserved when possible. This is a powerful feature for development, enabling a live coding style. It can be used to build long-running servers that update logic on the fly by loading new Flow libraries.

* **Controls and Metacommands:** The Flow language includes controls (the second parameter in the CLI usage). Controls are categories or subsystems. For instance, http is a control in the flowlang library that handles listening and request parsing. The flowmcp binary effectively uses flows as methods in a JSON-RPC service. Other controls (e.g., for scheduling, I/O) act as built-in commands. The built-in flowlang library contains commands like flowlang://http/parse_request used by the HTTP server. These act as plugins providing system functionality to flows (for example, to interface with networking). Create custom control categories in libraries to organize commands.

* **Peer-to-Peer and Newbound Context:** Flow was originally part of a system aimed at peer-to-peer web apps (Newbound). Flowlang's design allows multiple instances (peers) to run and exchange flow definitions or data. Environments like Newbound use Flowlang for both server-side (Rust interpreter) and client-side logic (e.g., a JS Flow interpreter). The crate itself doesn't implement networking (besides the basic HTTP server and MCP via stdio), but it is designed for embedding in distributed systems.

* **Experimental "Mirror" Mode:** A mirror feature flag exists. This relates to running Flowlang in a mirrored memory mode, facilitating multi-process setups. When enabled, Flowlang allocates the ndata heap in shared memory or a named region so it can be mirrored. This feature flag is for specific advanced use cases.

* **Future Directions:** Because Flowlang uses JSON as its meta-language, tools can generate Flow JSON from other representations (for example, translating a subset of Python or a visual builder into Flow JSON). The core interpreter executes these without change. The crate is under active development (v0.3.x as of the time of this writing) and future versions will introduce optimizations (like multi-threaded execution of independent subgraphs, or a JIT compiler for flows). The architecture accommodates such changes due to the clear separation of the flow model and execution engine. The continued development of MCP support will further solidify its role in AI and agent-based systems.

In conclusion, Flowlang offers a compelling way to design systems by **wiring together dataflow components**. It stands out by bridging multiple languages in one runtime and by providing Rust developers a dynamic, visual scripting layer. With the addition of flowmcp and its strong suitability for LLM tooling, Flowlang is an increasingly powerful platform for building sophisticated, modern applications. By understanding its internal model (Cases, Operations, Connections) and following best practices (proper initialization, memory management, and leveraging native code where appropriate), developers can harness Flowlang to build flexible applications that can even be modified at runtime. Whether used via a visual IDE like Newbound or directly as a Rust crate, Flowlang demonstrates how Rust's power can be extended to support a high-level, cross-language programming paradigm.
