use std::env;
use std::io::{self, BufRead, Write};
//use ndata::json_util;
use ndata::dataobject::DataObject;
use flowlang::init;
use flowlang::appserver::init_globals;
use flowlang::mcp::mcp::mcp::mcp;

#[cfg(feature = "gag")]
use gag::Gag;

fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    {
        init("data");
        init_globals();

        eprintln!("[flow] MCP flow started — expecting JSON-RPC over stdin/stdout");

        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut stdout_lock = stdout.lock();

        for line in stdin.lock().lines() {
            match line {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }

                    eprintln!("[flow] Incoming line: {}", line);

                    match DataObject::try_from_string(&line) {
                        Ok(query) => {
                            if !query.has("id") || !query.get_property("id").is_number() {
                                eprintln!("[flow] NOT GONNA DO IT JACK: {}", query.to_string());
                                continue;
                            }

                            let response = {
                                #[cfg(feature = "gag")]
                                let _stdout_gag = Gag::stdout().unwrap();
                                mcp(query)
                            };

                            eprintln!("[flow] Outgoing response: {}", response.to_string());
                            writeln!(stdout_lock, "{}", response.to_string()).unwrap();
                            stdout_lock.flush().unwrap();
                        }
                        Err(err) => {
                            eprintln!("[flow] Error during request: {}", err);
                            let msg = format!("Internal error: {}", err);
                            let mut error = DataObject::new();
                            error.put_int("code", -32603);
                            error.put_string("message", &msg);
                            let mut fallback = DataObject::new();
                            fallback.put_string("jsonrpc","2.0");
                            fallback.put_null("id");
                            fallback.put_object("error", error);
                            writeln!(stdout_lock, "{}", fallback.to_string()).unwrap();
                            stdout_lock.flush().unwrap();
                        }
                    }
                }
                Err(err) => {
                    eprintln!("[flow] Error reading stdin: {}", err);
                    break;
                }
            }
        }
    }

    eprintln!("[flow] Stream closed. Exiting MCP flow.");
}
