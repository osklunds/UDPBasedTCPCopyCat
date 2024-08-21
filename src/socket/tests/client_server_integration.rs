
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

    assert_eq!(client_addr, client_stream.local_addr().unwrap());
    assert_eq!(server_addr, server_stream.local_addr().unwrap());
    assert_ne!(client_addr, server_addr);

    write_and_read(&mut server_stream, &mut client_stream, b"hello from server");
    write_and_read(&mut client_stream, &mut server_stream, b"hello from client");

    write_and_read(&mut client_stream, &mut server_stream, b"a");
    write_and_read(&mut server_stream, &mut client_stream, b"b");

    write_and_read(&mut client_stream, &mut server_stream, b"short msg");
    write_and_read(&mut server_stream, &mut client_stream, b"loooooooong messe");

    client_stream.shutdown();
    server_stream.shutdown();
    client_stream.wait_shutdown_complete();
    server_stream.wait_shutdown_complete();
    listener.shutdown_all();
    listener.wait_shutdown_complete();
}

fn write_and_read(writer_stream: &mut Stream, reader_stream: &mut Stream, data: &[u8]) {
    let mut read_data = vec![0; data.len()];

    writer_stream.write(data).unwrap();

    assert_ne!(read_data, data);
    reader_stream.read_exact(&mut read_data).unwrap();
    assert_eq!(read_data, data);
}

