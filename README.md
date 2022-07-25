# Flow
### The Flowlang crate - https://crates.io/crates/flowlang
This repo is published on crates.io and provides a unified Functional Programming API 
across functions written in Rust, Java, JavaScript, Python and Flow.

### The Flow Language
The Flow language is a 3D visual dataflow language, which is based loosely on the Prograph programming language 
(https://en.wikipedia.org/wiki/Prograph). Flow allows you to construct a diagram of how data flows through your 
application, and then execute it. The official IDE for the Flow language is Newbound 
(https://github.com/mraiser/newbound). 

### Introductory Video:
[![Watch the video](https://img.youtube.com/vi/j7S5__ObWis/maxresdefault.jpg)](https://youtu.be/5vZKR4FGJyU)
https://youtu.be/5vZKR4FGJyU

### Installation
This repo can be used as a binary or a library. I assume if you are using this to develop something and want to 
publish something for release, you already know how to convert the below instructions from "debug" to "release". 
To compile and use as a binary (on Linux):

    git clone https://github.com/mraiser/flow.git flow
    cd flow
    cargo build
    mypath=$(pwd)
    cd /usr/bin
    sudo ln -s $mypath/target/debug/flow flow
    sudo ln -s $mypath/target/debug/flowb flowb
    cd -

To use as a Rust library, add the following to your Cargo.toml file:

    [dependencies]
    flowlang = "0.1.8"
    # NOTE: Change version to latest version: https://crates.io/crates/flowlang

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
    env::set_var("RUST_BACKTRACE", "1");
    {
        let args = DataObject::from_json(serde_json::from_str(r#"
        {
          "a": 299,
          "b": 121
        }
        "#).unwrap());
        let cmd = Command::lookup("testflow", "testflow", "test_add");
        let res = cmd.execute(args).unwrap();
        println!("Hello, my dudes! {}", res.to_json());
    }
    DataStore::gc();

#### From a web browser:
    # Start the HTTP service from the directory where you installed Flow
    flow flowlang http listen <<< "{\"socket_address\": \"127.0.0.1:7878\", \"library\":\"flowlang\", \"control\":\"http\", \"command\":\"parse_request\"}"
Test your HTTP service in a web browser:

http://127.0.0.1:7878/testflow/testflow/test_add?a=42&b=378

### Support for commands in multiple languages
Flow commands can be written in Java, Python, Rust, Javascript, or Flow. All languages maintain state 
between calls. When developing Flow code using Newbound, the IDE automatically builds, compiles, and runs any files 
needed. Newbound has its own instructions for enabling support for multiple languages 
(https://github.com/mraiser/newbound). The following only applies to running Flow code *outside* of the Newbound IDE.

#### Enabling JavaScript commands:
Since JavaScript support is a feature that is disabled by default, you will have to compile flow with the 
`--features=javascript_runtime` flag.

    cargo run --features javascript_runtime --bin flow testflow testflow test_javascript <<< "{\"a\":\"world\"}"

#### Enabling Python commands:
Since Python support is a feature that is disabled by default, you will have to compile flow with the
`--features=python_runtime` flag. You must install Python3 in the local environment first. In order to 
run Libraries that contain commands written in Python, you will need to generate the Python files.

    # To rebuild all rust and python commands, use: flowb all
    cargo run --bin flowb testflow testflow test_python
    cargo run --features python_runtime --bin flow testflow testflow test_python <<< "{\"a\":\"world\"}"

#### Enabling Rust commands:
In order to run Libraries that contain commands written in Rust, you will need to generate the Rust files.

    # To rebuild all rust and python commands, use: flowb all
    cargo run --bin flowb testflow testflow test_rust
    cargo run --bin flow testflow testflow test_rust <<< "{\"a\":\"world\"}"

#### Enabling Java commands
In order to run Libraries that contain commands written in Java, you will need to add data/botmanager, 
runtime/botmanager, runtime/peerbot, src/Startup.java, src/com, and src/org from Newbound 
(https://github.com/mraiser/newbound) to your Flow project. Since Java support is a feature that is disabled by 
default, you will have to compile flow with the `--features=java_runtime` flag. You will also need to make sure 
the JDK's libjvm library is in your `LD_LIBRARY_PATH`.

    mkdir bin
    mkdir runtime
    git clone https://github.com/mraiser/newbound.git newbound
    cp -R newbound/data/botmanager data/botmanager
    cp -R newbound/runtime/botmanager runtime/botmanager
    cp -R newbound/runtime/peerbot runtime/peerbot
    cp newbound/src/Startup.java src/Startup.java
    cp -R newbound/src/com src/com
    cp -R newbound/src/org src/org
    cd src
    javac -d ../bin Startup.java
    cd ../
    # Make sure LD_LIBRARY_PATH contains path to libjvm.so 
    # Something along the lines of:
    export LD_LIBRARY_PATH=/usr/lib/jvm/java-11-openjdk-amd64/lib/server/
    # Example from testflow library:
    cargo run --bin flow --features=java_runtime testflow testflow test_java <<< "{\"a\":\"world\"}"

### Background:
Flow was originally written in Java as part of Newbound, an integrated development environment for peer-to-peer HTML5 
web apps. Newbound supports Java, JavaScript, Python, Rust, and Flow for server-side commands, and Javascript and Flow 
on the front-end. This repository contains a port of the Flow language interpreter from Newbound's Java implementation
(https://github.com/mraiser/newbound/blob/master/runtime/botmanager/src/com/newbound/code/Code.java). Newbound also 
uses this crate to compile and execute Rust code.
