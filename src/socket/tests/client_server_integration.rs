
use super::*;

use async_std::future;
use std::io::{Error, ErrorKind};
use std::net::{UdpSocket, *};
use std::time::Duration;
use std::cmp;

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

#[test]
fn multiple_clients() {
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    //////////////////////////////////////////////////////////////////
    // Bind
    //////////////////////////////////////////////////////////////////

    let mut listener = Listener::bind(initial_addr).unwrap();
    let server_addr = listener.local_addr().unwrap();

    //////////////////////////////////////////////////////////////////
    // Connect client 1
    //////////////////////////////////////////////////////////////////

    let connect_client_thread1 = thread::spawn(move || {
        Stream::connect(server_addr).unwrap()
    });
    let (mut server_stream1, client1_addr) = listener.accept().unwrap();
    let mut client_stream1 = connect_client_thread1.join().unwrap();

    assert_eq!(client1_addr, client_stream1.local_addr().unwrap());
    assert_eq!(server_addr, server_stream1.local_addr().unwrap());
    assert_ne!(client1_addr, server_addr);

    //////////////////////////////////////////////////////////////////
    // Connect client 2
    //////////////////////////////////////////////////////////////////

    let connect_client_thread2 = thread::spawn(move || {
        Stream::connect(server_addr).unwrap()
    });
    let (mut server_stream2, client2_addr) = listener.accept().unwrap();
    let mut client_stream2 = connect_client_thread2.join().unwrap();

    assert_eq!(client2_addr, client_stream2.local_addr().unwrap());
    assert_eq!(server_addr, server_stream2.local_addr().unwrap());
    assert_ne!(client2_addr, server_addr);

    assert_ne!(client2_addr, client1_addr);

    //////////////////////////////////////////////////////////////////
    // Read and write
    //////////////////////////////////////////////////////////////////

    write_and_read(&mut server_stream1, &mut client_stream1, b"from server 1 to client 1");
    write_and_read(&mut server_stream2, &mut client_stream2, b"from server 2 to client 2");
    write_and_read(&mut client_stream1, &mut server_stream1, b"from client 1 to server 1");
    write_and_read(&mut client_stream2, &mut server_stream2, b"from client 2 to server 2");

    //////////////////////////////////////////////////////////////////
    // Connect client 3
    //////////////////////////////////////////////////////////////////

    let connect_client_thread3 = thread::spawn(move || {
        Stream::connect(server_addr).unwrap()
    });
    let (mut server_stream3, client3_addr) = listener.accept().unwrap();
    let mut client_stream3 = connect_client_thread3.join().unwrap();

    assert_eq!(client3_addr, client_stream3.local_addr().unwrap());
    assert_eq!(server_addr, server_stream3.local_addr().unwrap());
    assert_ne!(client3_addr, server_addr);

    assert_ne!(client3_addr, client1_addr);
    assert_ne!(client3_addr, client2_addr);

    //////////////////////////////////////////////////////////////////
    // Read and write
    //////////////////////////////////////////////////////////////////

    write_and_read(&mut server_stream1, &mut client_stream1, b"one");
    write_and_read(&mut server_stream2, &mut client_stream2, b"two");
    write_and_read(&mut server_stream3, &mut client_stream3, b"three");
    write_and_read(&mut client_stream1, &mut server_stream1, b"ett");
    write_and_read(&mut client_stream2, &mut server_stream2, b"tva");
    write_and_read(&mut client_stream3, &mut server_stream3, b"tre");

    //////////////////////////////////////////////////////////////////
    // Shutdown connection 1
    //////////////////////////////////////////////////////////////////

    client_stream1.shutdown();
    server_stream1.shutdown();
    client_stream1.wait_shutdown_complete();
    server_stream1.wait_shutdown_complete();

    //////////////////////////////////////////////////////////////////
    // Read and write
    //////////////////////////////////////////////////////////////////

    write_and_read(&mut server_stream2, &mut client_stream2, b"hej");
    write_and_read(&mut client_stream2, &mut server_stream2, b"hello");

    //////////////////////////////////////////////////////////////////
    // Shutdown connection 2
    //////////////////////////////////////////////////////////////////

    client_stream2.shutdown();
    server_stream2.shutdown();
    client_stream2.wait_shutdown_complete();
    server_stream2.wait_shutdown_complete();

    //////////////////////////////////////////////////////////////////
    // Shutdown connection 3
    //////////////////////////////////////////////////////////////////

    client_stream3.shutdown();
    server_stream3.shutdown();
    client_stream3.wait_shutdown_complete();
    server_stream3.wait_shutdown_complete();

    listener.shutdown_all();
    listener.wait_shutdown_complete();
}

#[test]
fn random_simultaneous_reads_and_writes_high_load() {
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let mut listener = Listener::bind(initial_addr).unwrap();
    let server_addr = listener.local_addr().unwrap();

    let client_writes = random_datas(1000);
    let all_client_data = concat_datas(&client_writes);

    let server_writes = random_datas(1000);
    let all_server_data = concat_datas(&server_writes);

    let client_thread = thread::spawn(move || {
        let mut client_stream = Stream::connect(server_addr).unwrap();

        for data in client_writes {
            client_stream.write(&data).unwrap();
        }

        let mut read_data = vec![0; all_server_data.len()];
        client_stream.read_exact(&mut read_data).unwrap();

        assert_eq!(all_server_data, read_data);
        client_stream.shutdown();
        client_stream.wait_shutdown_complete();
    });

    let (mut server_stream, _client_addr) = listener.accept().unwrap();

    let server_thread = thread::spawn(move || {
        for data in server_writes {
            server_stream.write(&data).unwrap();
        }

        let mut read_data = vec![0; all_client_data.len()];
        server_stream.read_exact(&mut read_data).unwrap();

        assert_eq!(all_client_data, read_data);


        server_stream.shutdown();
        server_stream.wait_shutdown_complete();
    });

    client_thread.join().unwrap();
    server_thread.join().unwrap();

    listener.shutdown_all();
    listener.wait_shutdown_complete();
}

#[test]
fn one_client_proxy() {
    let localhost_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();

    let mut listener = Listener::bind(localhost_addr).unwrap();
    let server_addr = listener.local_addr().unwrap();

    let proxy_socket = UdpSocket::bind(localhost_addr).unwrap();
    let proxy_addr = proxy_socket.local_addr().unwrap();

    let _proxy_thread = thread::spawn(move || {
        let mut client_addr = None;

        loop {
            let mut buf = [0; 4096];
            let (amt, recv_addr) = proxy_socket.recv_from(&mut buf).unwrap();
            let recv_data = &buf[0..amt];

            let segment = Segment::decode(recv_data).unwrap();
            println!("{:?}", segment);

            // Data from server
            if recv_addr == server_addr {
                proxy_socket.send_to(recv_data, client_addr.unwrap()).unwrap();
            }
            // Data from client
            else {
                // client addr can't be known at start. Only when client
                // connects does it become known
                if client_addr == None {
                    client_addr = Some(recv_addr);
                } else {
                    assert_eq!(Some(recv_addr), client_addr);
                }

                proxy_socket.send_to(recv_data, server_addr).unwrap();
            }
        }
    });

    let connect_client_thread = thread::spawn(move || {
        Stream::connect(proxy_addr).unwrap()
    });

    let (mut server_stream, _client_addr_from_server) = listener.accept().unwrap();
    let mut client_stream = connect_client_thread.join().unwrap();

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

////////////////////////////////////////////////////////////////////////////////
// Helper functions
////////////////////////////////////////////////////////////////////////////////

fn write_and_read(writer_stream: &mut Stream, reader_stream: &mut Stream, data: &[u8]) {
    let mut read_data = vec![0; data.len()];

    writer_stream.write(data).unwrap();

    assert_ne!(read_data, data);
    reader_stream.read_exact(&mut read_data).unwrap();
    assert_eq!(read_data, data);
}

fn random_datas(number_of_datas: u32) -> Vec<Vec<u8>> {
    let mut datas = Vec::new();

    for _ in 0..number_of_datas {
        let len = cmp::max(rand::random::<u32>() % MAXIMUM_SEGMENT_SIZE, 1);
        datas.push(random_data_of_length(len));
    }

    datas
}

fn random_data_of_length(length: u32) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::new();
    for _ in 0..length {
        data.push(rand::random());
    }
    data
}

fn concat_datas(datas: &Vec<Vec<u8>>) -> Vec<u8> {
    let mut all = Vec::new();

    for data in datas {
        all.extend_from_slice(data);
    }

    all
}

