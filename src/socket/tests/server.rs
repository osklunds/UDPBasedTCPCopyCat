
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
    let server_addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
    let listener = Listener::bind(server_addr).unwrap();

    let server_thread = thread::spawn(move || {
        let _server_socket = listener.accept().unwrap();
        println!("{:?}", "accepted");
    });

    // Connect
    let local_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let client_socket = UdpSocket::bind(local_addr).unwrap();
    UdpSocket::connect(&client_socket, server_addr).unwrap();

    let send_next = 77;
    let syn = Segment::new_empty(Syn, send_next, 0);
    let encoded_syn = Segment::encode(&syn);

    client_socket.send(&encoded_syn).unwrap();

    // Receive SYN-ACK
    let mut buf = [0; 4096];
    let amt = client_socket.recv(&mut buf).unwrap();

    let syn_ack = Segment::decode(&buf[0..amt]).unwrap();

    let new_send_next = send_next + 1;
    assert_eq!(new_send_next, syn_ack.ack_num());
    assert_eq!(SynAck, syn_ack.kind());
    assert_eq!(0, syn_ack.data().len());

    let receive_next = syn_ack.seq_num() + 1;

    // Send ACK
    let ack = Segment::new_empty(Ack, new_send_next, receive_next);
    let encoded_ack = Segment::encode(&ack);

    client_socket.send(&encoded_ack).unwrap();

    println!("{:?}", "connected");


    server_thread.join().unwrap();

    println!("{:?}", "done");

    ()
}
