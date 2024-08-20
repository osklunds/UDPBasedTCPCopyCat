#![allow(non_snake_case)]

use super::*;

use async_std::future;
use std::io::{Error, ErrorKind};
use std::net::{UdpSocket, *};
use std::time::Duration;

use self::mock_timer::MockTimer;
use crate::segment::Segment;

#[test]
fn mf_explicit_sequence_numbers() {
    let initial_server_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = Listener::bind(initial_server_addr).unwrap();
    let server_addr = listener.local_addr().unwrap();
    println!("\n\n server addr {:?}   \n\n\n\n", server_addr);

    let server_thread = thread::spawn(move || {
        println!("{:?}\n\n\n\n", "call accept");
        let server_socket = listener.accept().unwrap();
        println!("{:?}", "accepted");
        server_socket
    });

    // Connect
    let local_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let client_socket = UdpSocket::bind(local_addr).unwrap();
    UdpSocket::connect(&client_socket, server_addr).unwrap();

    // Send SYN
    let syn = Segment::new_empty(Syn, 2000, 0);
    let encoded_syn = Segment::encode(&syn);
    client_socket.send(&encoded_syn).unwrap();

    // Receive SYN-ACK
    let mut buf = [0; 4096];
    let amt = client_socket.recv(&mut buf).unwrap();
    let syn_ack = Segment::decode(&buf[0..amt]).unwrap();

    assert_eq!(2001, syn_ack.ack_num());
    assert_eq!(SynAck, syn_ack.kind());
    assert_eq!(0, syn_ack.data().len());

    let send_next = syn_ack.seq_num() + 1;

    // Send ACK
    let ack = Segment::new_empty(Ack, 2001, send_next);
    let encoded_ack = Segment::encode(&ack);
    client_socket.send(&encoded_ack).unwrap();

    println!("{:?}", "connected");

    let (mut server_socket, _client_addr) = server_thread.join().unwrap();

    println!("{:?}", "tc accept");

    //////////////////////////////////////////////////////////////////
    // Write #1
    //////////////////////////////////////////////////////////////////

    server_socket.write(b"hello").unwrap();

    let exp_seg_write1 = Segment::new(Ack, 1001, 2001, b"hello");
    let seg_write1 = recv_segment_from(&client_socket, server_addr);
    assert_eq!(exp_seg_write1, seg_write1);

    let ack_seg_write1 = Segment::new_empty(Ack, 2001, 1006);
    send_segment_to(&client_socket, server_addr, &ack_seg_write1);

    println!("{:?}", "done");

    ()
}

fn recv_segment_from(tc_socket: &UdpSocket, uut_addr: SocketAddr) -> Segment {
    let (seg, recv_addr) = recv_segment_with_addr(tc_socket);
    assert_eq!(uut_addr, recv_addr);
    seg
}

fn recv_segment_with_addr(tc_socket: &UdpSocket) -> (Segment, SocketAddr) {
    let mut buf = [0; 4096];
    let (amt, recv_addr) = tc_socket.recv_from(&mut buf).unwrap();
    (Segment::decode(&buf[0..amt]).unwrap(), recv_addr)
}

fn send_segment_to(
    tc_socket: &UdpSocket,
    uut_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    tc_socket.send_to(&encoded_seq, uut_addr).unwrap();
}
