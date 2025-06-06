# **Flowlang**

## **Purpose and Core Functionality**

**Flowlang** is a Rust implementation of the **Flow language**, a
dataflow-oriented programming language designed for visual \"flow\"
diagrams. The crate\'s primary purpose is to **execute Flow programs**
(defined in JSON) and provide a unified function-call interface across
multiple programming languages, including Rust, Python, JavaScript, and
Java. In essence, Flowlang acts as an **interpreter and runtime** for
Flow programs, allowing developers to construct programs as dataflow
graphs and run them seamlessly. This addresses the problem of
orchestrating complex logic in a visual, data-driven manner, and
integrating code written in different languages into one workflow.

Its multi-language support and inherent dataflow paradigm make Flowlang
particularly well-suited for **building and orchestrating Large Language
Model (LLM) based tools and agents**. Developers can seamlessly
integrate Python scripts for LLM interactions, Rust for
performance-critical tasks, and JavaScript for other utilities, all
within a unified visual workflow. The *flowmcp* binary further enhances
this by providing direct support for the Model Control Protocol.

A Flow program is represented as a directed graph of operations
(\"commands\") where data flows along connections between nodes. The
Flow language is loosely based on Prograph, a 3D visual dataflow
language. Using an IDE like **Newbound**, a developer draws a diagram of
how data moves through functions and conditions; Flowlang then executes
this diagram by passing data through the graph. Each node (operation)
processes inputs and produces outputs that feed into other nodes. The
Flowlang crate essentially interprets the JSON representation of such a
diagram, allowing it to run as a program.

One of Flowlang\'s distinctive features is **multi-language support**.
It provides a unified functional API so that \"Flow commands\" (nodes in
the flow graph) can be implemented not only in Flow\'s own visual
language but also in **Rust, Python, JavaScript, or Java**. This means
developers can write certain nodes as native Rust functions, or as
Python/JS scripts, etc., and integrate them into the dataflow. The
Flowlang runtime handles calling out to the correct language runtime and
feeding data in/out, which simplifies building heterogeneous systems.
All these languages maintain state between calls, so for example the
Python interpreter or JavaScript engine isn\'t re-initialized on every
use, enabling persistent stateful behavior across multiple calls.

**Relation to ndata:** The Flowlang crate is built on top of the
companion crate *ndata*, which provides the dynamic data structures used
to represent and pass data between flow nodes. *ndata* defines types
like *Data*, *DataObject*, and *DataArray* that behave similarly to
loosely-typed JSON values or Python objects. These can hold numbers,
strings, booleans, nested objects/arrays, etc., and are used as the
universal data container in Flowlang. Crucially, *ndata* implements an
**internal heap with reference counting and garbage collection**. This
allows Flowlang to create and pass around dynamic data (e.g., the input
and output parameters to commands) without worrying about Rust\'s strict
ownership rules---much like a garbage-collected language. In practice,
every input or output in a flow is a *DataObject* (a JSON-like map of
keys to *Data* values) that can be freely shared across threads and
languages. The Flowlang runtime leverages *ndata* so that data flows
smoothly through the graph, regardless of which language produced or
consumes it. This design choice makes Flowlang **thread-safe by design**
as *ndata*\'s objects use internal reference counts and locks so they
can be sent between threads without explicit *Arc* wrappers. In summary,
Flowlang\'s core functionality is enabling dataflow programming
(especially visual programming via Newbound) and seamless multi-language
function integration, built atop a dynamic data model provided by
*ndata*. This empowers rapid prototyping and cross-language development
by abstracting away memory management and language interop complexities.

## **Flowlang as a Premier Platform for LLM Tooling and Model Control Protocol (MCP)**

With the rise of Large Language Models (LLMs), the need for robust and
flexible tooling to orchestrate LLM interactions, chain prompts, manage
state, and integrate with various APIs has become paramount. Flowlang,
with its inherent strengths, is exceptionally positioned as the **best
vehicle for rolling your own LLM tools and agents**, especially with the
introduction of Model Control Protocol (MCP) support via the *flowmcp*
binary.

**What is Model Control Protocol (MCP)?** MCP provides a standardized
way for applications to communicate with and control AI models or
agents. It involves sending structured requests (often JSON-RPC) to a
model endpoint and receiving structured responses. This allows for
complex interactions beyond simple prompt-response, including managing
context, controlling model parameters, and invoking specific agent
capabilities.

**Introducing ***flowmcp***:** The *flowmcp* binary in Flowlang is a
dedicated executable that implements an MCP server. It listens for
JSON-RPC messages over stdin, processes them using the Flowlang engine,
and sends responses back via stdout. This allows external systems or
interfaces to interact with Flowlang-defined workflows as if they were
language models or intelligent agents.

**Why Flowlang is Ideal for LLM Tooling:**

1.  **Seamless Multi-Language Integration:**

    -   **Python Dominance in LLMs:** The majority of LLM SDKs (e.g.,
        OpenAI, Hugging Face Transformers, LangChain, LlamaIndex) are
        Python-based. Flowlang\'s first-class Python support (via
        *PyO3*) allows direct embedding of Python scripts as nodes in a
        flow. This means leveraging existing LLM libraries and custom
        Python code for model interaction, prompt templating, and data
        processing without complex FFI wrappers.
    -   **Rust for Performance:** For pre-processing, post-processing,
        or any performance-critical logic in an LLM pipeline, native
        Rust commands can be written.
    -   **JavaScript & Java:** Integration with web APIs or existing
        Java/.js libraries is also supported.

2.  **Visual Dataflow Programming for Complex Chains:**

    -   LLM applications often involve complex chains of operations:
        fetching data, constructing prompts, calling an LLM, parsing the
        response, making decisions, calling another LLM or tool, and so
        on.
    -   Representing these chains as visual Flow diagrams (e.g., in
        Newbound) makes them significantly easier to design, understand,
        debug, and modify compared to monolithic scripts.
    -   The dataflow paradigm naturally maps to how data (prompts,
        responses, context) moves through an LLM agent.

3.  **Flexible Data Handling with ***ndata***:**

    -   LLM inputs and outputs can be complex JSON structures.
        *ndata*\'s *DataObject* provides a flexible, JSON-like way to
        handle this data dynamically across different language
        components in a flow.

4.  **State Management:**

    -   Flowlang\'s ability to maintain state within language runtimes
        (e.g., Python interpreter, JS engine) between calls is crucial
        for LLM applications that require conversational memory or
        persistent context.
    -   Global variables within Flowlang can also be used to manage
        shared state across different parts of an LLM agent\'s logic.

5.  **Rapid Prototyping and Iteration:**

    -   The visual nature and multi-language support accelerate the
        prototyping of LLM tools. Different models, prompt strategies,
        or processing logic can be quickly swapped by modifying the flow
        graph or the underlying scripts.

6.  **Exposing LLM Tools as Services:**

    -   With *flowmcp*, sophisticated Flowlang-orchestrated LLM agents
        can be exposed over a standardized JSON-RPC interface.
    -   Additionally, Flowlang\'s built-in HTTP server allows easy
        conversion of LLM flows into web services.

**Example Use Case: A Research Agent Flow** Imagine an LLM agent that
takes a research query, searches the web, summarizes relevant articles,
and generates a report. In Flowlang:

-   An initial node (Python) uses a search engine API.
-   Multiple parallel nodes (Python) call an LLM to summarize each
    article.
-   A subsequent node (Python or Rust) synthesizes these summaries.
-   A final node (Python) uses an LLM to generate the final report based
    on the synthesis.
-   *flowmcp* allows an external application to invoke this entire
    research agent with a single JSON-RPC call.

By leveraging Flowlang and *flowmcp*, developers can build powerful,
modular, and maintainable LLM-powered applications with greater ease and
clarity than traditional scripting approaches.

## **Key Technologies and Design (Rust Features & Concurrency)**

Despite being implemented in Rust, Flowlang adopts many techniques more
common in dynamic or functional languages. Key Rust technologies and
design choices include:

-   **Dynamic Data with Manual GC:** Flowlang uses the *ndata* crate to
    manage data dynamically. *ndata* internally uses a global heap and
    manual garbage collection---unusual for Rust, but deliberate here to
    allow more flexibility. All *DataObject* and *DataArray* instances
    carry their own reference counts, and memory is only freed when a GC
    function is explicitly called. This means Flowlang can store cyclic
    or cross-scope data (e.g., global state or interconnected node
    outputs) without immediate ownership issues. The trade-off is that
    the programmer (or the runtime) must periodically invoke
    *DataStore::gc()* (which calls *NData::gc()*) to clean up unused
    values. This design restores some of the \"garbage-collected
    language\" convenience inside Rust\'s safe environment, at the cost
    of forgoing Rust\'s usual compile-time memory guarantees. It\'s a
    conscious choice to make Flowlang suitable for **rapid prototyping**
    and multi-language interop. In practice, when writing Rust code that
    uses Flowlang, **do not wrap Flow data in additional ***Arc*** or
    ***Mutex*****---*ndata* already handles thread-safe reference
    counting internally. A common mistake is to put *Data* or
    *DataObject* inside an *Arc*; this is unnecessary and could lead to
    memory never being freed (as *ndata*\'s GC would not see the data as
    collectable). Instead, rely on Flowlang/*ndata*\'s own memory model
    and simply call the GC when appropriate (for example, after a batch
    of flow executions, call *DataStore::gc()* to reclaim heap storage).

-   **Thread-Safety and Concurrency Model:** Flowlang\'s concurrency
    model is built around the idea that flows can run in parallel, but
    individual flow executions are single-threaded by default. The Flow
    interpreter uses an event-loop style algorithm to evaluate the
    dataflow graph (detailed in the next section) and does not spawn
    multiple threads for parallel nodes---instead, it processes nodes
    whose inputs are ready in sequence. However, because *ndata* data
    structures are thread-safe, it is possible to run multiple Flow
    **commands (functions)** concurrently in different threads or tasks.
    For example, two separate *Command::execute* calls can happen on
    different threads---the underlying data passing (using *DataObject*)
    is protected by atomic reference counts and locks, so data races
    will not occur. In short, Flowlang itself doesn\'t automatically
    parallelize a single flow, but it *allows multi-threaded use*. The
    thread safety is achieved without heavy use of *Mutex* thanks to the
    internal design of *ndata*: references to data are coordinated by a
    custom thread-safe reference counter (*SharedMutex* in *ndata*) so
    that cloning a *DataObject* just bumps a count and different threads
    can read/write through it safely. This simplifies concurrent
    scenarios---manual copying or guarding of flow inputs/outputs to
    share them is not needed. The Flowlang interpreter loop also uses
    only safe Rust (no *unsafe* for concurrency), leaning on the atomic
    refcounts for synchronization. There is no explicit use of Rust
    *async/await* in Flowlang; flows are generally run to completion
    synchronously via *Command::execute*. If asynchronous behavior is
    needed (e.g., waiting on I/O), implement that inside a node (for
    instance, a Rust node can use *tokio* internally, or a JavaScript
    node can *await* a promise in the embedded engine).

-   **FFI and Language Embedding:** Under the hood, Flowlang leverages
    **Rust\'s FFI capabilities** to integrate other language runtimes:

    -   **JavaScript:** It includes an optional feature to embed the
        Deno/V8 engine. The crate depends on *deno_core* and *serde_v8*;
        when the *javascript_runtime* feature is enabled, Flowlang
        spawns a V8 isolate (via Deno\'s core) to execute JS code. Each
        JS-based flow command is run in this engine, with data passed
        through JSON serialization (*serde_v8* bridges Rust *DataObject*
        to V8 values).
    -   **Python:** Flowlang uses *pyo3* (via a *python_runtime*
        feature) to embed a Python 3 interpreter. When this feature is
        enabled, Flowlang provides a direct, high-performance bridge
        between Rust\'s *ndata* types and Python. Instead of converting
        data to native Python dictionaries and lists on every call,
        Flowlang passes **handles** to the underlying Rust data. In
        Python, these are exposed as *NDataObject*, *NDataArray*, and
        *NDataBytes* classes. This approach avoids costly serialization
        overhead and allows Python code to manipulate the same
        underlying data that Rust sees, making cross-language calls
        highly efficient. Python-defined flow commands are executed in
        the same interpreter, maintaining state (e.g., global variables,
        imported modules) between calls. This is particularly crucial
        for LLM tooling, where Python is prevalent.
    -   **Java:** Flowlang employs the Java Native Interface (JNI)
        (*jni* crate) when *java_runtime* is enabled. Java support is
        the most involved, requiring specific Java helper classes (e.g.,
        *Startup.java* and associated packages) to be present in the
        classpath. If configured, Flowlang loads the JVM (requiring
        *libjvm.so* or its equivalent to be on the system\'s library
        path, e.g., *LD_LIBRARY_PATH*) and can call Java methods for
        flow commands.
    -   **Rust (native) functions:** Flowlang has a special mechanism
        for native functions. Rather than FFI, Rust commands are
        compiled into the project and registered. The Flowlang crate
        includes a separate binary called *flowb* (\"flow builder\"),
        which generates Rust source stubs for any Flow commands meant to
        be implemented in Rust and compiles them into the project.
        Internally this is handled by a module that registers Rust
        function pointers. These Rust commands are then invoked directly
        when their node executes, which is highly efficient.

All these integrations highlight Rust\'s ability to host multiple
runtimes simultaneously. Flowlang uses conditional compilation (feature
flags) to keep these optional---by default, only pure Flow and Rust are
supported, and one compiles with *\--features=javascript_runtime* or
others to include JS, Python, or Java support. This modular design keeps
the base crate lightweight and lets users opt-in only to the needed
language engines.

-   **Macros and Code Generation:** The Flowlang codebase itself
    doesn\'t rely heavily on procedural macros, but it does generate
    code at build-time for Rust commands. When *flowb* is run, it
    programmatically writes out a Rust source file containing stubs to
    call user-defined Rust functions and a registry of those functions.
    This file is included via *mod cmdinit* in the crate. At runtime,
    the crate calls a function (generated in that module) to register
    these commands. This approach uses Rust\'s compile-time generation
    rather than a macro, but the effect is similar to a plugin system.
-   **Error Handling and Control Flow:** The interpreter uses Rust
    *Result* and a custom *CodeException* enum for internal control
    flow. For example, if a node signals a failure or a termination, the
    interpreter returns a *CodeException::Fail* or *::Terminate* which
    unwinds the execution loop in a controlled way. This is how
    Flow-level control structures like \"stop flow\" or \"goto next
    case\" are implemented. Rust\'s *match* and error handling are used
    here instead of exceptions; but conceptually, they serve a similar
    role to propagate events like \"skip to next branch\" up to the main
    loop. This design keeps the core loop clean and avoids deeply nested
    conditionals.

In summary, Flowlang\'s architecture is an interesting blend: it
sacrifices some of Rust\'s usual strictness (using a global heap and
dynamic typing) to gain flexibility, while still leveraging Rust\'s
strengths in FFI, speed, and safety for multi-language support. The
concurrency model is cooperative and data-driven---multiple languages
run in the same event loop and thread, unless they are explicitly
threaded out. The design emphasizes that data is the primary carrier of
state (fitting a dataflow paradigm), and everything from memory
management to multi-language calls is built to make passing around
*DataObject* instances simple and safe.

## **Installation and Usage**

Flowlang can be used both as a **standalone binary** and as a
**library** crate in a Rust project. Depending on the use case,
installation can be done either way:

-   **As a Binary (CLI Tool):** The crate comes with three binaries:
    *flow* (the main interpreter), *flowb* (the builder for Rust/Python
    commands), and *flowmcp* (for Model Control Protocol interactions).
    Obtain these by cloning the GitHub repo and building:

    *git clone https://github.com/mraiser/flow.git*

    *cd flow*

    *cargo build \# builds the flow, flowb, and flowmcp binaries*

    *\# (Optionally, copy or symlink the binaries to a directory in your
    PATH)*

    *sudo ln -s \$(pwd)/target/debug/flow /usr/local/bin/flow*

    *sudo ln -s \$(pwd)/target/debug/flowb /usr/local/bin/flowb*

    *sudo ln -s \$(pwd)/target/debug/flowmcp /usr/local/bin/flowmcp*

    This compiles the latest code. (For a release build, use *cargo
    build \--release* and adjust the paths accordingly.) Once built, the
    *flow* CLI can execute Flow libraries. By default, it looks for a
    *data* directory in the current working directory which contains the
    flow libraries (JSON files). The repository itself includes a
    *data/* folder with an example library called **\"testflow\"**.

-   **To run a flow from the command line with ***flow*****, use:

    *flow \<library\> \<control\> \<command\> \<\<\< \'\<json-input\>\'*

    For example, to execute the *test_add* command in the *testflow*
    library:

    *cd path/to/flow \# directory containing \'data\' folder*

    *flow testflow testflow test_add \<\<\< \'{\"a\": 300, \"b\":
    120}\'*

    This launches the Flow interpreter, loads the **testflow** library,
    and runs the function named **test_add** with the provided JSON
    input. The result is printed to stdout as JSON.

-   **To use ***flowmcp*** for Model Control Protocol interactions:**
    The *flowmcp* binary starts a server that listens for JSON-RPC
    requests on stdin and sends responses to stdout.

    *\# Run flowmcp (it will wait for JSON-RPC requests on stdin)*

    *./target/debug/flowmcp*

    An external application would then pipe JSON-RPC requests like the
    following to *flowmcp*\'s stdin:

    *{\"jsonrpc\": \"2.0\", \"method\": \"testflow.testflow.test_add\",
    \"params\": {\"a\": 5, \"b\": 7}, \"id\": 1}*

    And *flowmcp* would respond on stdout:

    *{\"jsonrpc\":\"2.0\",\"result\":{\"result\":12},\"id\":1}*

-   **As a Library in Rust:** Include Flowlang in a Cargo project by
    adding to **Cargo.toml**:

    *\[dependencies\]*

    *flowlang = \"0.3.21\" \# Or the latest version*

    *ndata = \"0.3.13\" \# Or the version compatible with your flowlang*

    With this, the Flow runtime can be initialized and flows executed
    from Rust code. A minimal example:

    *use flowlang::datastore::DataStore;*

    *use flowlang::command::Command;*

    *use ndata::dataobject::DataObject;*

    *use flowlang::init; // For flowlang::init*

    *fn main() {*

    *// Initialize the Flow runtime with the path to the data
    libraries:*

    *init(\"data\"); // Recommended: sets up DataStore and registers
    Rust commands*

    *std::env::set_var(\"RUST_BACKTRACE\", \"1\");*

    *// Prepare input as a DataObject (from JSON string):*

    *let args_json = r#\"{\"a\": 299, \"b\": 121}\"#;*

    *let args = DataObject::try_from_string(args_json).expect(\"Failed
    to parse JSON input\");*

    *// Lookup the command by library, category, and name:*

    *let cmd = Command::lookup(\"testflow\", \"testflow\",
    \"test_add\").expect(\"Command not found\");*

    *// Execute the command:*

    *match cmd.execute(args) {*

    *Ok(result) =\> {*

    *println!(\"Result = {}\", result.to_string());*

    *}*

    *Err(e) =\> {*

    *eprintln!(\"Flow execution error: {:?}\", e);*

    *}*

    *}*

    *DataStore::gc(); // optional: run garbage collection*

    *}*

-   **HTTP Service Usage:** Flowlang has a built-in mini HTTP server
    that can expose flow commands as web endpoints.

    *flow flowlang http listen \<\<\< \'{\"socket_address\":
    \"127.0.0.1:7878\", \...}\'*

    This command starts an HTTP listener. Any Flow command can then be
    invoked via an HTTP *GET* request.

-   **Enabling Language Runtimes:** If flows include commands written in
    other languages, compilation must include the corresponding
    features:

    -   **JavaScript:** *cargo run \--features \"javascript_runtime\"
        \--bin flow \...*
    -   **Python:** *cargo run \--features \"python_runtime\" \...*
    -   **Java:** *cargo run \--features \"java_runtime\" \...*

## **Code Structure and Flow Execution Architecture**

Internally, Flowlang represents a flow as a collection of interconnected
components.

-   **Modules Organization:** The crate is divided into several modules:
    *datastore*, *command*, *code*, *case*, *primitives*, *rustcmd*,
    *pycmd*, *jscmd*, *javacmd*, *mcp*, *buildrust*, and various utility
    modules.

-   **Flow Definition Data Structures:** When a flow library (JSON) is
    loaded, it is parsed into a set of in-memory structs:

    -   *****Case*****: Represents a flow function\'s code---a
        collection of operations and connections.
    -   *****Operation*****: Represents a single operation/node in the
        flow graph.
    -   *****Node*****: Represents a data port on a *Case* or
        *Operation*.
    -   *****Connection*****: Represents a directed link from a source
        *Node* to a destination *Node*.

-   **Loading and Parsing Flows:** *init(\"data\")* reads the library
    JSON files from the *data* directory and builds the in-memory *Case*
    structures, registering each as an executable *Command*.

-   **Interpreter Execution Algorithm:** The heart of Flowlang is
    *Code::execute*, which runs a *Case*. It uses a two-phase event
    loop:

    -   **Operation Pass:** Iterates through all operations, checking if
        their inputs are satisfied. If so, it executes the operation\'s
        logic (calling Rust, Python, etc.).
    -   **Connection Pass:** Iterates through all connections,
        propagating data from completed source nodes to their
        destination nodes. This process repeats until no more operations
        can fire and no more data can be propagated, effectively
        performing a topological sort of the graph on the fly. The
        interpreter is single-threaded but allows for thread-safe data
        access via *ndata*.

## **Examples and Best Practices**

**Example:** Suppose a flow is needed to compute *a \* b + c*. This can
be done by writing a Rust function or assembling a Flow visually. A
simplified JSON for such a flow would define inputs *a*, *b*, and *c*,
two primitive operations (*multiply*, *add*), and connections to wire
them together correctly.

**Best Practices & Caveats:**

-   **Memory Management:** Run *DataStore::gc()* periodically in
    long-running services to prevent memory bloat, as *ndata*\'s garbage
    collection is manual.

-   **No External Sync Needed:** Do not wrap *ndata* types
    (*DataObject*, *DataArray*, etc.) in *Arc* or *Mutex*. They are
    already internally thread-safe.

-   **Global State:** Use *DataStore::globals()* for state that needs to
    persist across flow invocations.

-   **Using Multi-Language Commands (especially for LLMs):** When
    writing flow commands in other languages, ensure the initialization
    steps are followed.

    -   **Python:** When using the *python_runtime* feature, your Python
        code will receive arguments as instances of the *NDataObject*,
        *NDataArray*, and *NDataBytes* classes. These are Pythonic
        wrappers around the underlying Rust data. Your Python functions
        should manipulate these objects directly and can return them to
        Flowlang. If a simple type (like a string or integer) is
        returned, Flowlang will automatically wrap it in a *DataObject*.
        This avoids performance penalties from data conversion and
        allows for a more natural coding experience.
    -   **JavaScript:** Enable *javascript_runtime*. The JS code runs in
        a Deno/V8 sandbox.
    -   **Rust:** After writing a Rust function for a command, use
        *flowb* to generate the necessary stubs and register it with the
        runtime.

-   **Performance Considerations:** Flowlang is optimized for
    flexibility. For performance-critical logic, implement it as a
    single native Rust command rather than a large graph of many small
    operations. Be mindful of the FFI overhead when crossing language
    boundaries frequently.

-   **Debugging Flows:** Set *RUST_BACKTRACE=1* for backtraces on
    panics. Use *eprintln!* for logging in *flowmcp* to avoid
    interfering with JSON-RPC output on stdout.

## **Extensions and Advanced Features**

-   **Plugins/Custom Libraries:** Extend Flowlang by adding new JSON
    flow definitions to the *data* directory.
-   **Hot Reloading:** Flowlang integrates with the *hot-lib-reloader*
    crate to allow live code updates without restarting the runtime,
    enabling a live-coding development style.
-   **Controls and Metacommands:** Use controls (e.g., *flowlang http
    \...*) to access built-in system-level functionality.
-   **Peer-to-Peer and Newbound Context:** The architecture is suitable
    for embedding in distributed or peer-to-peer systems.

In conclusion, Flowlang offers a compelling way to design systems by
**wiring together dataflow components**. It stands out by bridging
multiple languages in one runtime and by providing Rust developers a
dynamic, visual scripting layer. With the addition of *flowmcp* and its
strong suitability for LLM tooling, Flowlang is an increasingly powerful
platform for building sophisticated, modern applications.
