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

struct State {
    udp_socket: UdpSocket,
    stream: Stream,
    peer_addr: SocketAddr,
    seq_num: u32,
    ack_num: u32,
}

#[test]
fn test_connect() {
    setup_connected_client();
}

fn setup_connected_client() -> State {
    let server_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();
    let server_udp_socket = UdpSocket::bind(server_addr).unwrap();

    connect(server_udp_socket)
}

fn connect(server_udp_socket: UdpSocket) -> State {
    // Connect
    let server_addr = server_udp_socket.local_addr().unwrap();
    let client_stream = Stream::connect(&server_addr).unwrap();

    // Receive SYN
    let (syn, client_addr) = recv_segment_from(&server_udp_socket);
    assert_eq!(Syn, syn.kind());

    // Send SYN-ACK
    let mut server_seq_num = rand::random();
    let client_seq_num = syn.seq_num() + 1;
    let syn_ack = Segment::new(SynAck, server_seq_num, client_seq_num, &vec![]);
    send_segment(&server_udp_socket, client_addr, &syn_ack);

    // Receive ACK
    let ack = recv_segment(&server_udp_socket, client_addr);
    assert_eq!(Ack, ack.kind());
    server_seq_num += 1;
    assert_eq!(server_seq_num, ack.ack_num());

    State {
        udp_socket: server_udp_socket,
        stream: client_stream,
        peer_addr: client_addr,
        seq_num: server_seq_num,
        ack_num: client_seq_num,
    }
}

#[test]
fn test_client_read_once() {
    let mut state = setup_connected_client();

    read(&mut state, "some data");
}

fn read(state: &mut State, string: &str) {
    let data = string.as_bytes();

    // Send from the server
    let send_seg = Segment::new(Ack, state.seq_num, state.ack_num, data);
    send_segment(&state.udp_socket, state.peer_addr, &send_seg);
    state.seq_num += data.len() as u32;
    // Check that the server received an ACK
    let recv_seg = recv_segment(&state.udp_socket, state.peer_addr);
    let exp_ack = Segment::new(Ack, state.ack_num, state.seq_num, &vec![]);
    assert_eq!(exp_ack, recv_seg);

    // Check that the client received the correct data
    let read_data = read_stream(&mut state.stream);
    assert_eq!(data, read_data);
}

#[test]
fn test_client_read_multiple_times() {
    let mut state = setup_connected_client();

    read(&mut state, "first rweouinwrte");
    read(&mut state, "second hfuiasud");
    read(&mut state, "third uifdshufihsiughsyudfghkusfdf");
    read(&mut state, "fourth fuidshfadgaerge");
    read(&mut state, "fifth dhuifghuifdlfoiwejiow");
    read(&mut state, "sixth fdauykfudsfgs");
    read(&mut state, "seventh fsdhsdgfsd");
    read(&mut state, "eighth ijogifdgire");
    read(&mut state, "ninth ertwrw");
    read(&mut state, "tenth uhfsdghsu");
}

#[test]
fn test_client_write_once() {
    let mut state = setup_connected_client();

    write(&mut state, "some data");
}

fn write(state: &mut State, string: &str) {
    let data = string.as_bytes();

    // Send from the client
    let written_len = state.stream.write(&data).unwrap();
    let len = data.len();
    assert_eq!(len, written_len);

    // Check that the server received a segment with the data
    let recv_seg = recv_segment(&state.udp_socket, state.peer_addr);
    let exp_seg = Segment::new(Ack, state.ack_num, state.seq_num, &data);
    assert_eq!(exp_seg, recv_seg);

    // // Send from the server
    // let send_seg = Segment::new(Ack, state.seq_num, state.ack_num, data);
    // send_segment(&state.udp_socket, state.peer_addr, &send_seg);
    // state.seq_num += data.len() as u32;

    // // Check that the server received an ACK
    // let recv_seg = recv_segment(&state.udp_socket, state.peer_addr);
    // let exp_ack = Segment::new(Ack, state.ack_num, state.seq_num, &vec![]);
    // assert_eq!(exp_ack, recv_seg);

    // // Check that the client received the correct data
    // let read_data = read_stream(&mut state.stream);
    // assert_eq!(data, read_data);
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

fn read_stream(stream: &mut Stream) -> Vec<u8> {
    let mut buf = [0; 4096];
    let amt = stream.read(&mut buf).unwrap();
    buf[0..amt].to_vec()
}

#[test]
fn my_test() {}
