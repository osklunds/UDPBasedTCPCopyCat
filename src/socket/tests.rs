use super::*;

use std::net::*;

#[test]
fn test() {
    let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();

    let server_to_client = "hello from server".as_bytes();
    let client_to_server = "hi from client".as_bytes();

    let server_thread = thread::spawn(move || {
        // Listen
        let listener = Listener::bind(server_addr).unwrap();
        println!("{:?}", "listening");

        // Accept
        let (mut server_socket, _peer_addr) = listener.accept().unwrap();
        println!("{:?}", "accepted");

        // Write
        let amt_write = server_socket.write(&server_to_client).unwrap();
        assert_eq!(server_to_client.len(), amt_write);
        println!("{:?}", "server write done");

        // Read
        let mut buf = [0; 4096];
        let amt_read = server_socket.read(&mut buf).unwrap();
        let read = &buf[0..amt_read];
        assert_eq!(client_to_server, read);
        println!("{:?}", "server read done");
    });

    // Connect
    let mut client_socket = Stream::connect(server_addr).unwrap();
    println!("{:?}", "connected");

    // Read
    let mut buf = [0; 4096];
    let amt_read = client_socket.read(&mut buf).unwrap();
    let read = &buf[0..amt_read];
    assert_eq!(server_to_client, read);
    println!("{:?}", "client read done");

    // Write
    let amt_write = client_socket.write(&client_to_server).unwrap();
    assert_eq!(client_to_server.len(), amt_write);
    println!("{:?}", "client write done");

    server_thread.join().unwrap();

    println!("{:?}", "done");

    ()
}
