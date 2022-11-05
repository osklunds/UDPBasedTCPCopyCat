use super::*;

use std::net::*;

use crate::segment::Segment;

// #[test]
// fn test() {
//     let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();

//     let server_to_client = "hello from server".as_bytes();
//     let client_to_server = "hi from client".as_bytes();

//     let server_thread = thread::spawn(move || {
//         // Listen
//         let listener = Listener::bind(server_addr).unwrap();
//         println!("{:?}", "listening");

//         // Accept
//         let (mut server_socket, _peer_addr) = listener.accept().unwrap();
//         println!("{:?}", "accepted");

//         // Write
//         let amt_write = server_socket.write(&server_to_client).unwrap();
//         assert_eq!(server_to_client.len(), amt_write);
//         println!("{:?}", "server write done");

//         // Read
//         let mut buf = [0; 4096];
//         let amt_read = server_socket.read(&mut buf).unwrap();
//         let read = &buf[0..amt_read];
//         assert_eq!(client_to_server, read);
//         println!("{:?}", "server read done");
//     });

//     // Connect
//     let mut client_socket = Stream::connect(server_addr).unwrap();
//     println!("{:?}", "connected");

//     // Read
//     let mut buf = [0; 4096];
//     let amt_read = client_socket.read(&mut buf).unwrap();
//     let read = &buf[0..amt_read];
//     assert_eq!(server_to_client, read);
//     println!("{:?}", "client read done");

//     // Write
//     let amt_write = client_socket.write(&client_to_server).unwrap();
//     assert_eq!(client_to_server.len(), amt_write);
//     println!("{:?}", "client write done");

//     server_thread.join().unwrap();

//     println!("{:?}", "done");

//     ()
// }

#[test]
fn test_client_connect_full_handshake() {
    let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();
    let server_udp_socket = UdpSocket::bind(server_addr).unwrap();

    // Connect
    let _client_stream = Stream::connect(&server_addr).unwrap();

    // Receive SYN
    let mut buf = [0; 4096];
    let (amt_syn, client_addr_syn) =
        server_udp_socket.recv_from(&mut buf).unwrap();
    let syn = Segment::decode(&buf[0..amt_syn]).unwrap();
    assert_is_handshake_syn(&syn);

    // Send SYN-ACK
    let seq_num = rand::random();
    let syn_ack =
        Segment::new(true, true, false, seq_num, syn.seq_num() + 1, &vec![]);
    let encoded_syn_ack = syn_ack.encode();
    server_udp_socket
        .send_to(&encoded_syn_ack, client_addr_syn)
        .unwrap();

    // Receive ACK
    let (amt_ack, client_addr_ack) =
        server_udp_socket.recv_from(&mut buf).unwrap();
    let ack = Segment::decode(&buf[0..amt_ack]).unwrap();
    assert_is_handshake_ack(&ack);
    assert_eq!(seq_num + 1, ack.ack_num());
    assert_eq!(client_addr_syn, client_addr_ack);
}

fn assert_is_handshake_syn(segment: &Segment) {
    assert_eq!(true, segment.syn());
    assert_eq!(false, segment.ack());
    assert_eq!(false, segment.fin());
    assert_eq!(0, segment.data().len());
}

fn assert_is_handshake_ack(segment: &Segment) {
    assert_eq!(false, segment.syn());
    assert_eq!(true, segment.ack());
    assert_eq!(false, segment.fin());
    assert_eq!(0, segment.data().len());
}
