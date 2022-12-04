use super::*;

use std::net::{UdpSocket, *};
use std::time::Duration;

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
    tc_socket: UdpSocket,
    uut_stream: Stream,
    uut_addr: SocketAddr,
    tc_seq_num: u32,
    uut_seq_num: u32,
}

#[test]
fn test_connect() {
    setup_connected_uut_client();
}

fn setup_connected_uut_client() -> State {
    let tc_addr: SocketAddr = "127.0.0.1:6789".parse().unwrap();
    let tc_socket = UdpSocket::bind(tc_addr).unwrap();

    uut_connect(tc_socket)
}

fn uut_connect(tc_socket: UdpSocket) -> State {
    // Connect
    let tc_addr = tc_socket.local_addr().unwrap();
    let uut_stream = Stream::connect(&tc_addr).unwrap();

    // Receive SYN
    let (syn, uut_addr) = recv_segment_from(&tc_socket);
    assert_eq!(Syn, syn.kind());

    // Send SYN-ACK
    let mut tc_seq_num = rand::random();
    let uut_seq_num = syn.seq_num() + 1;
    let syn_ack = Segment::new(SynAck, tc_seq_num, uut_seq_num, &vec![]);
    send_segment(&tc_socket, uut_addr, &syn_ack);

    // Receive ACK
    let ack = recv_segment(&tc_socket, uut_addr);
    assert_eq!(Ack, ack.kind());
    tc_seq_num += 1;
    assert_eq!(tc_seq_num, ack.ack_num());

    State {
        tc_socket,
        uut_stream,
        uut_addr,
        tc_seq_num,
        uut_seq_num,
    }
}

#[test]
fn test_client_read_once() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, "some data");
}

fn uut_read(state: &mut State, string: &str) {
    let data = string.as_bytes();

    // Send from the tc
    let send_seg = Segment::new(Ack, state.tc_seq_num, state.uut_seq_num, data);
    send_segment(&state.tc_socket, state.uut_addr, &send_seg);
    state.tc_seq_num += data.len() as u32;

    // Recv ACK from the uut
    let recv_seg = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_ack =
        Segment::new(Ack, state.uut_seq_num, state.tc_seq_num, &vec![]);
    assert_eq!(exp_ack, recv_seg);

    // Check that the uut received the correct data
    let read_data = uut_read_stream(&mut state.uut_stream);
    assert_eq!(data, read_data);
}

#[test]
fn test_client_read_multiple_times() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, "first rweouinwrte");
    uut_read(&mut state, "second hfuiasud");
    uut_read(&mut state, "third uifdshufihsiughsyudfghkusfdf");
    uut_read(&mut state, "fourth fuidshfadgaerge");
    uut_read(&mut state, "fifth dhuifghuifdlfoiwejiow");
    uut_read(&mut state, "sixth fdauykfudsfgs");
    uut_read(&mut state, "seventh fsdhsdgfsd");
    uut_read(&mut state, "eighth ijogifdgire");
    uut_read(&mut state, "ninth ertwrw");
    uut_read(&mut state, "tenth uhfsdghsu");
}

#[test]
fn test_client_write_once() {
    let mut state = setup_connected_uut_client();

    uut_write(&mut state, "some data");
}

fn uut_write(state: &mut State, string: &str) {
    let data = string.as_bytes();

    // Send from the uut
    let written_len = state.uut_stream.write(&data).unwrap();
    let len = data.len();
    assert_eq!(len, written_len);

    // Recv from the tc
    let recv_seg = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg = Segment::new(Ack, state.uut_seq_num, state.tc_seq_num, &data);
    assert_eq!(exp_seg, recv_seg);
    state.uut_seq_num += len as u32;

    // Send ack from the tc
    let send_seg =
        Segment::new(Ack, state.tc_seq_num, state.uut_seq_num, &vec![]);
    send_segment(&state.tc_socket, state.uut_addr, &send_seg);

    // TODO: Check buffer size
}

#[test]
fn test_client_write_multiple_times() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, "first agfs");
    uut_read(&mut state, "second gfdhdgfh");
    uut_read(&mut state, "third dfafsdfads");
    uut_read(&mut state, "fourth dfafas");
    uut_read(&mut state, "fifth dfasfasfsdaf");
    uut_read(&mut state, "sixth thythrt");
    uut_read(&mut state, "seventh fdsaref");
    uut_read(&mut state, "eighth dagfsdrgrege");
    uut_read(&mut state, "ninth asfaerger");
    uut_read(&mut state, "tenth trehjk");
}

fn recv_segment(tc_socket: &UdpSocket, uut_addr: SocketAddr) -> Segment {
    let (seg, recv_addr) = recv_segment_from(tc_socket);
    assert_eq!(uut_addr, recv_addr);
    seg
}

fn recv_segment_from(tc_socket: &UdpSocket) -> (Segment, SocketAddr) {
    let mut buf = [0; 4096];
    let (amt, recv_addr) = tc_socket.recv_from(&mut buf).unwrap();
    (Segment::decode(&buf[0..amt]).unwrap(), recv_addr)
}

fn send_segment(
    tc_socket: &UdpSocket,
    uut_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    tc_socket.send_to(&encoded_seq, uut_addr).unwrap();
}

fn uut_read_stream(stream: &mut Stream) -> Vec<u8> {
    let mut buf = [0; 4096];
    let amt = stream.read(&mut buf).unwrap();
    buf[0..amt].to_vec()
}
