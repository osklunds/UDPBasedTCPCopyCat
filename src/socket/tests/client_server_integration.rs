
use super::*;

use async_std::future;
use std::io::{Error, ErrorKind};
use std::net::{UdpSocket, *};
use std::time::Duration;

#[test]
fn one_client() {
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let mut listener = Listener::bind(initial_addr).unwrap();
    let server_addr = listener.local_addr().unwrap();

    let connect_client_thread = thread::spawn(move || {
        Stream::connect(server_addr).unwrap()
    });

    let (mut server_stream, client_addr) = listener.accept().unwrap();

    let mut client_stream = connect_client_thread.join().unwrap();

    // assert_eq!(client_addr, client_stream.local_addr());
    assert_ne!(client_addr, server_addr);

    const DATA_TO_SEND: &[u8; 17] = b"hello from client";
    client_stream.write(DATA_TO_SEND).unwrap();
    let mut data_received = [0; DATA_TO_SEND.len()];
    server_stream.read_exact(&mut data_received).unwrap();
    assert_eq!(data_received, *DATA_TO_SEND);

    client_stream.shutdown();
    server_stream.shutdown();
    client_stream.wait_shutdown_complete();
    server_stream.wait_shutdown_complete();
    listener.shutdown_all();
    listener.wait_shutdown_complete();
}
