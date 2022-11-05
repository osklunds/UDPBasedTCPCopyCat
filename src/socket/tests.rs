use super::*;

use std::net::*;

#[test]
fn test() {
    let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();

    let server_to_client = "hello from server".as_bytes();

    let server_thread = thread::spawn(move || {
        let listener = Listener::bind(server_addr).unwrap();
        println!("{:?}", "listening");

        let (mut server_socket, _peer_addr) = listener.accept().unwrap();
        println!("{:?}", "accepted");

        let amt = server_socket.write(&server_to_client).unwrap();
        assert_eq!(server_to_client.len(), amt);
    });

    let mut client_socket = Stream::connect(server_addr).unwrap();
    println!("{:?}", "connected");

    let mut buf = [0; 4096];
    let amt = client_socket.read(&mut buf).unwrap();
    let received = &buf[0..amt];
    assert_eq!(server_to_client, received);

    server_thread.join().unwrap();

    println!("{:?}", "done");

    ()
}
