//! The shop's server: what it listens on, and how long it waits.

use std::time::Duration;

const LISTEN_PORT: u16 = 8080;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const REGION: &str = "nl";

fn main() {
    println!("listening on port {LISTEN_PORT} for region {REGION}, {REQUEST_TIMEOUT:?} a request");
}
