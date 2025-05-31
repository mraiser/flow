use std::env;
use flowlang::init;
use flowlang::appserver::init_globals;
use flowlang::mcp::mcp::mcp::run;

#[cfg(feature = "gag")]
use gag::Gag;

fn main() {
    env::set_var("RUST_BACKTRACE", "1");
    {
        init("data");
        init_globals();

        run();
    }
}
