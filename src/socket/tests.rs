use super::*;

use std::net::*;

#[test]
fn test() {
    let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();

    let server_thread = thread::spawn(move || {
        let listener = Listener::bind(server_addr).unwrap();
        println!("{:?}", "listening");

        let server_socket = listener.accept();
        println!("{:?}", "accepted");
    });

    let client_socket = Stream::connect(server_addr);
    println!("{:?}", "connected");

    server_thread.join().unwrap();

    println!("{:?}", "done");

    ()
}
