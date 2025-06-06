use ::flowlang::*;

use std::env;
use init;
use appserver::init_globals;
use mcp::mcp::mcp::run;

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
