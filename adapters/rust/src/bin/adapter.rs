//! The Rust adapter of docattest: the program the docattest core starts to answer what
//! needs Rust. It speaks version 1 of the adapter protocol on its standard input and
//! output. Nobody runs it by hand.

use docattest_rust::protocol::{Adapter, serve};
use docattest_rust::{language, session};

fn main() {
    let adapter = Adapter { hello: language::hello(), language: language::answer, open: session::open };
    serve(std::io::stdin().lock(), std::io::stdout(), adapter);
}
