# Flow
*This repo is still very much a work in progress.*

The Flow language is a 3D visual dataflow language, which is based loosely on the Prograph programming language 
(https://en.wikipedia.org/wiki/Prograph). Flow was originally written in Java as part of Newbound, an integrated 
development and runtime environment for peer-to-peer HTML5 web apps. Newbound supports Java, Python and Flow for 
server-side commands, and Javascript and Flow on the front-end. This repository contains a port of the Flow language 
interpreter from the original Java, Python and Javascript versions included with Newbound.

- Java: https://github.com/mraiser/newbound/blob/master/runtime/botmanager/src/com/newbound/code/Code.java
- Python: https://github.com/mraiser/newbound/blob/master/runtime/botmanager/src/newbound/code/code.py
- Javascript: https://github.com/mraiser/newbound/blob/master/data/flow/nzsk/xq17/a964/97b3/nzskxq17a96497b37x14.js

### Introductory Video:
[![Watch the video](https://img.youtube.com/vi/j7S5__ObWis/maxresdefault.jpg)](https://youtu.be/zwC-_ZmbOfA)
https://youtu.be/zwC-_ZmbOfA

### Compile flow as binary
    cargo build --release

### Executing Flow Code
This repo includes a "data" folder which contains the "testflow" library. You can add your own libraries to the "data" 
folder, and they will become executable as well. Libraries are created using the Newbound Metabot 
(https://github.com/mraiser/newbound).

#### From the command line:
    cargo run --bin flow testflow testflow test_add <<< "{\"a\": 300,\"b\":120}"

#### From Rust code:
    DataStore::init("data");

    let args = DataObject::from_json(serde_json::from_str(r#"
    {
      "a": 299,
      "b": 121
    }
    "#).unwrap());
    let cmd = Command::lookup("testflow", "testflow", "test_add);
    let res = cmd.execute(args).unwrap();
    println!("Hello, my dudes! {}", res.to_json());

#### From a web browser:
    # Start the HTTP service
    flow flowlang http listen <<< "{\"socket_address\": \"127.0.0.1:7878\", \"library\":\"flowlang\", \"control\":\"http\", \"command\":\"parse_request\"}"
Test your HTTP service in a web browser:
    
http://127.0.0.1:7878/testflow/testflow/test_speed?a=100000
