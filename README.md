# Flow

The Flow language is a 3D visual dataflow language, which is based loosely on the Prograph programming language 
(https://en.wikipedia.org/wiki/Prograph). The official IDE for the Flow language is Newbound 
(https://github.com/mraiser/newbound). 
### Introductory Video:
[![Watch the video](https://img.youtube.com/vi/j7S5__ObWis/maxresdefault.jpg)](https://youtu.be/5vZKR4FGJyU)
https://youtu.be/5vZKR4FGJyU

### Installation
This repo can be used as a binary or a library. To compile and use as a binary (on Linux):

    cargo build --release
    sudo cp target/release/flow /usr/bin/flow
    sudo cp target/release/flowb /usr/bin/flowb

To use as a Rust library, add the following to your Cargo.toml file:

    [dependencies]
    flow = { path = "../../rust/flow" }
    # NOTE: Change path to the relative path of where you installed Flow

To use as a native library in Java (on Linux), add libflow.so to your Java library path. Then add a native class in 
Java, like this one: https://github.com/mraiser/newbound/blob/master/runtime/botmanager/src/com/newbound/code/LibFlow.java

    cargo build --release
    sudo cp target/release/libflow.so /usr/lib/jni/libflow.so

### Executing Flow Code
This repo includes a "data" folder which contains the "testflow" library. You can add your own libraries to the "data" 
folder, and they will become executable as well. Libraries are created using the Newbound Metabot 
(https://github.com/mraiser/newbound).

#### From the command line:

Execute the following from the directory that contains the "data" directory containing your Flow code.

    flow testflow testflow test_add <<< "{\"a\": 300,\"b\":120}"

#### From Rust code:
    DataStore::init("data");
    Generated::init(); // Load any flow commands written in rust
    {
        let args = DataObject::from_json(serde_json::from_str(r#"
        {
          "a": 299,
          "b": 121
        }
        "#).unwrap());
        let cmd = Command::lookup("testflow", "testflow", "test_add);
        let res = cmd.execute(args).unwrap();
        println!("Hello, my dudes! {}", res.to_json());
    }
    DataStore::gc();

#### From a web browser:
    # Start the HTTP service from the directory where you installed Flow
    flow flowlang http listen <<< "{\"socket_address\": \"127.0.0.1:7878\", \"library\":\"flowlang\", \"control\":\"http\", \"command\":\"parse_request\"}"
Test your HTTP service in a web browser:

http://127.0.0.1:7878/testflow/testflow/test_add?a=42&b=378

### Support for other languages
Flow commands can be written in Java, Python, Rust, Javascript, or Flow. However *Java, Python, and Javascript are not 
currently supported* in this implementation of the Flow language interpreter. 

Compiling Rust commands:

    # From the directory containing the "data" directory with your flow code that has rust commands
    flowb all
    cargo build --release

### Background:
Flow was originally written in Java as part of Newbound, an integrated
development and runtime environment for peer-to-peer HTML5 web apps. Newbound supports Java, Python, Rust, and Flow for
server-side commands, and Javascript and Flow on the front-end. This repository contains a port of the Flow language
interpreter from Newbound's Java implementation. Newbound also uses this repo to compile and execute Rust code.

- Java: https://github.com/mraiser/newbound/blob/master/runtime/botmanager/src/com/newbound/code/Code.java
- Python: https://github.com/mraiser/newbound/blob/master/runtime/botmanager/src/newbound/code/code.py
- Javascript: https://github.com/mraiser/newbound/blob/master/data/flow/nzsk/xq17/a964/97b3/nzskxq17a96497b37x14.js
