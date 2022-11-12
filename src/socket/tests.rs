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
fn test_client_connect() {
    let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();
    let server_udp_socket = UdpSocket::bind(server_addr).unwrap();

    client_connect(&server_udp_socket);
}

fn client_connect(
    server_udp_socket: &UdpSocket,
) -> (Stream, SocketAddr, u32, u32) {
    // Connect
    let server_addr = server_udp_socket.local_addr().unwrap();
    let client_stream = Stream::connect(&server_addr).unwrap();

    // Receive SYN
    let (syn, client_addr) = recv_segment_from(&server_udp_socket);
    assert_is_handshake_syn(&syn);

    // Send SYN-ACK
    let mut server_seq_num = rand::random();
    let client_seq_num = syn.seq_num() + 1;
    let syn_ack = Segment::new(SynAck, server_seq_num, client_seq_num, &vec![]);
    send_segment(&server_udp_socket, client_addr, &syn_ack);

    // Receive ACK
    let ack = recv_segment(&server_udp_socket, client_addr);
    assert_is_handshake_ack(&ack);
    server_seq_num += 1;
    assert_eq!(server_seq_num, ack.ack_num());

    (client_stream, client_addr, server_seq_num, client_seq_num)
}

fn assert_is_handshake_syn(segment: &Segment) {
    assert_eq!(Syn, segment.kind());
    assert_eq!(0, segment.data().len());
}

fn assert_is_handshake_ack(segment: &Segment) {
    assert_eq!(Ack, segment.kind());
    assert_eq!(0, segment.data().len());
}

#[test]
fn test_client_recv() {
    let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();
    let server_udp_socket = UdpSocket::bind(server_addr).unwrap();

    let (_client_stream, client_addr, mut server_num, client_num) =
        client_connect(&server_udp_socket);

    let data = "some data".as_bytes();
    let seg = Segment::new(Ack, server_num, client_num, data.clone());

    send_segment(&server_udp_socket, client_addr, &seg);
    server_num += data.len() as u32;

    let segment = recv_segment(&server_udp_socket, client_addr);
    let exp_ack_segment = Segment::new(Ack, client_num, server_num, &vec![]);
    assert_eq!(exp_ack_segment, segment);
}

fn recv_segment(udp_socket: &UdpSocket, peer_addr: SocketAddr) -> Segment {
    let (seg, recv_addr) = recv_segment_from(udp_socket);
    assert_eq!(peer_addr, recv_addr);
    seg
}

fn recv_segment_from(udp_socket: &UdpSocket) -> (Segment, SocketAddr) {
    let mut buf = [0; 4096];
    let (amt, recv_addr) = udp_socket.recv_from(&mut buf).unwrap();
    (Segment::decode(&buf[0..amt]).unwrap(), recv_addr)
}

fn send_segment(
    udp_socket: &UdpSocket,
    peer_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    udp_socket.send_to(&encoded_seq, peer_addr).unwrap();
}
