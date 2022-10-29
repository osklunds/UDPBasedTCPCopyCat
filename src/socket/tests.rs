use super::*;

use std::net::*;

#[test]
fn test() {
    let addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();

    let listener = Listener::bind(addr);

    ()
}
