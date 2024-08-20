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
    let initial_uut_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = Listener::bind(initial_uut_addr).unwrap();
    let uut_addr = listener.local_addr().unwrap();

    let accept_thread = thread::spawn(move || {
        println!("{:?}", "tc: start accept");
        let uut_stream = listener.accept().unwrap();
        println!("{:?}", "tc: accept done");
        uut_stream
    });

    //////////////////////////////////////////////////////////////////
    // Connect
    //////////////////////////////////////////////////////////////////

    let initial_tc_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tc_socket = UdpSocket::bind(initial_tc_addr).unwrap();
    let tc_addr = tc_socket.local_addr().unwrap();
    assert_ne!(tc_addr, uut_addr);

    UdpSocket::connect(&tc_socket, uut_addr).unwrap();

    // Send SYN
    let syn = Segment::new_empty(Syn, 2000, 0);
    send_segment_to(&tc_socket, uut_addr, &syn);

    // Receive SYN-ACK
    let syn_ack = recv_segment_from(&tc_socket, uut_addr);

    // TODO: 1000 is hard coded. Need to change
    let exp_syn_ack = Segment::new_empty(SynAck, 1000, 2001);
    assert_eq!(exp_syn_ack, syn_ack);

    // Send ACK
    let ack = Segment::new_empty(Ack, 2001, 1001);
    send_segment_to(&tc_socket, uut_addr, &ack);

    let (mut uut_stream, client_addr) = accept_thread.join().unwrap();
    assert_eq!(tc_addr, client_addr);

    println!("{:?}", "tc: connect done");

    //////////////////////////////////////////////////////////////////
    // Write #1
    //////////////////////////////////////////////////////////////////

    uut_stream.write(b"hello").unwrap();

    let exp_seg_write1 = Segment::new(Ack, 1001, 2001, b"hello");
    let seg_write1 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_seg_write1, seg_write1);

    let ack_seg_write1 = Segment::new_empty(Ack, 2001, 1006);
    send_segment_to(&tc_socket, uut_addr, &ack_seg_write1);

    println!("write1 done");

    //////////////////////////////////////////////////////////////////
    // Write #2
    //////////////////////////////////////////////////////////////////

    uut_stream.write(b"more").unwrap();

    let exp_seg_write2 = Segment::new(Ack, 1006, 2001, b"more");
    let seg_write2 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_seg_write2, seg_write2);

    let ack_seg_write2 = Segment::new_empty(Ack, 2001, 1010);
    send_segment_to(&tc_socket, uut_addr, &ack_seg_write2);

    println!("write2 done");
    
    //////////////////////////////////////////////////////////////////
    // Read
    //////////////////////////////////////////////////////////////////

    let seg_read1 = Segment::new(Ack, 2001, 1010, b"From test case");
    send_segment_to(&tc_socket, uut_addr, &seg_read1);

    let exp_ack_read1 = Segment::new_empty(Ack, 1010, 2015);
    let ack_read1 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_ack_read1, ack_read1);

    println!("read done");

    //////////////////////////////////////////////////////////////////
    // Shutdown from uut
    //////////////////////////////////////////////////////////////////

    uut_stream.shutdown();

    let exp_fin = Segment::new_empty(Fin, 1010, 2015);
    let fin_from_uut = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_fin, fin_from_uut);

    let ack_to_fin_from_uut = Segment::new_empty(Ack, 2015, 1011);
    send_segment_to(&tc_socket, uut_addr, &ack_to_fin_from_uut);

    //////////////////////////////////////////////////////////////////
    // Shutdown from tc
    //////////////////////////////////////////////////////////////////

    let fin_from_tc = Segment::new_empty(Fin, 2015, 1011);
    send_segment_to(&tc_socket, uut_addr, &fin_from_tc);

    let exp_ack_to_fin_from_tc = Segment::new_empty(Ack, 1011, 2016);
    let ack_to_fin_from_tc = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_ack_to_fin_from_tc, ack_to_fin_from_tc);

    uut_stream.wait_shutdown_complete();

    println!("{:?}", "done");

    ()
}

#[test]
fn mf_explicit_sequence_numbers_two_clients() {
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = Listener::bind(initial_addr).unwrap();
    let server_addr = listener.local_addr().unwrap();

    //////////////////////////////////////////////////////////////////
    // Connect client 1
    //////////////////////////////////////////////////////////////////

    let connect_client1_thread = thread::spawn(move || {
        let client1_socket = UdpSocket::bind(initial_addr).unwrap();
        let client1_addr = client1_socket.local_addr().unwrap();
        assert_ne!(client1_addr, server_addr);

        UdpSocket::connect(&client1_socket, server_addr).unwrap();

        // Send SYN
        let syn = Segment::new_empty(Syn, 2000, 0);
        send_segment_to(&client1_socket, server_addr, &syn);

        // Receive SYN-ACK
        let syn_ack = recv_segment_from(&client1_socket, server_addr);

        let exp_syn_ack = Segment::new_empty(SynAck, 1000, 2001);
        assert_eq!(exp_syn_ack, syn_ack);

        // Send ACK
        let ack = Segment::new_empty(Ack, 2001, 1001);
        send_segment_to(&client1_socket, server_addr, &ack);

        println!("{:?}", "tc: connect1 done");

        client1_socket
    });

    let (mut uut_stream1, client1_addr) = listener.accept().unwrap();
    let client1_socket = connect_client1_thread.join().unwrap();
    assert_eq!(client1_addr, client1_socket.local_addr().unwrap());
    
    //////////////////////////////////////////////////////////////////
    // Connect client 2
    //////////////////////////////////////////////////////////////////

    let connect_client2_thread = thread::spawn(move || {
        let client2_socket = UdpSocket::bind(initial_addr).unwrap();
        let client2_addr = client2_socket.local_addr().unwrap();
        assert_ne!(client2_addr, server_addr);

        UdpSocket::connect(&client2_socket, server_addr).unwrap();

        // Send SYN
        let syn = Segment::new_empty(Syn, 3000, 0);
        send_segment_to(&client2_socket, server_addr, &syn);

        // Receive SYN-ACK
        let syn_ack = recv_segment_from(&client2_socket, server_addr);

        let exp_syn_ack = Segment::new_empty(SynAck, 1000, 3001);
        assert_eq!(exp_syn_ack, syn_ack);

        // Send ACK
        let ack = Segment::new_empty(Ack, 3001, 1001);
        send_segment_to(&client2_socket, server_addr, &ack);

        println!("{:?}", "tc: connect2 done");

        client2_socket
    });

    let (mut uut_stream2, client2_addr) = listener.accept().unwrap();
    let client2_socket = connect_client2_thread.join().unwrap();
    assert_eq!(client2_addr, client2_socket.local_addr().unwrap());
    assert_ne!(client2_addr, client1_addr);

    //////////////////////////////////////////////////////////////////
    // Write #1 - client 1
    //////////////////////////////////////////////////////////////////

    uut_stream1.write(b"hello").unwrap();

    let exp_seg_write1 = Segment::new(Ack, 1001, 2001, b"hello");
    let seg_write1 = recv_segment_from(&client1_socket, server_addr);
    assert_eq!(exp_seg_write1, seg_write1);

    let ack_seg_write1 = Segment::new_empty(Ack, 2001, 1006);
    send_segment_to(&client1_socket, server_addr, &ack_seg_write1);

    println!("write1 client1 done");

    //////////////////////////////////////////////////////////////////
    // Write #1 - client 2
    //////////////////////////////////////////////////////////////////

    uut_stream2.write(b"hej").unwrap();

    let exp_seg_write1_client2 = Segment::new(Ack, 1001, 3001, b"hej");
    let seg_write1_client2 = recv_segment_from(&client2_socket, server_addr);
    assert_eq!(exp_seg_write1_client2, seg_write1_client2);

    let ack_seg_write1 = Segment::new_empty(Ack, 2001, 1004);
    send_segment_to(&client1_socket, server_addr, &ack_seg_write1);

    println!("write1 client2 done");

    //////////////////////////////////////////////////////////////////
    // Write #2
    //////////////////////////////////////////////////////////////////

    uut_stream1.write(b"more").unwrap();

    let exp_seg_write2 = Segment::new(Ack, 1006, 2001, b"more");
    let seg_write2 = recv_segment_from(&client1_socket, server_addr);
    assert_eq!(exp_seg_write2, seg_write2);

    let ack_seg_write2 = Segment::new_empty(Ack, 2001, 1010);
    send_segment_to(&client1_socket, server_addr, &ack_seg_write2);

    println!("write2 done");
    
    //////////////////////////////////////////////////////////////////
    // Read
    //////////////////////////////////////////////////////////////////

    let seg_read1 = Segment::new(Ack, 2001, 1010, b"From test case");
    send_segment_to(&client1_socket, server_addr, &seg_read1);

    let exp_ack_read1 = Segment::new_empty(Ack, 1010, 2015);
    let ack_read1 = recv_segment_from(&client1_socket, server_addr);
    assert_eq!(exp_ack_read1, ack_read1);

    println!("read done");

    //////////////////////////////////////////////////////////////////
    // Client1: Shutdown from uut
    //////////////////////////////////////////////////////////////////

    uut_stream1.shutdown();

    let exp_fin = Segment::new_empty(Fin, 1010, 2015);
    let fin_from_uut = recv_segment_from(&client1_socket, server_addr);
    assert_eq!(exp_fin, fin_from_uut);

    let ack_to_fin_from_uut = Segment::new_empty(Ack, 2015, 1011);
    send_segment_to(&client1_socket, server_addr, &ack_to_fin_from_uut);

    //////////////////////////////////////////////////////////////////
    // Client2: Shutdown from tc
    //////////////////////////////////////////////////////////////////

    let fin_from_tc = Segment::new_empty(Fin, 2015, 1011);
    send_segment_to(&client1_socket, server_addr, &fin_from_tc);

    let exp_ack_to_fin_from_tc = Segment::new_empty(Ack, 1011, 2016);
    let ack_to_fin_from_tc = recv_segment_from(&client1_socket, server_addr);
    assert_eq!(exp_ack_to_fin_from_tc, ack_to_fin_from_tc);

    uut_stream1.wait_shutdown_complete();

    //////////////////////////////////////////////////////////////////
    // Client2: Shutdown from tc
    //////////////////////////////////////////////////////////////////

    let fin_from_tc = Segment::new_empty(Fin, 3001, 1004);
    send_segment_to(&client2_socket, server_addr, &fin_from_tc);

    let exp_ack_to_fin_from_tc = Segment::new_empty(Ack, 1004, 3002);
    let ack_to_fin_from_tc = recv_segment_from(&client2_socket, server_addr);
    assert_eq!(exp_ack_to_fin_from_tc, ack_to_fin_from_tc);

    //////////////////////////////////////////////////////////////////
    // Client2: Shutdown from uut
    //////////////////////////////////////////////////////////////////

    uut_stream2.shutdown();

    let exp_fin = Segment::new_empty(Fin, 1004, 3002);
    let fin_from_uut = recv_segment_from(&client2_socket, server_addr);
    assert_eq!(exp_fin, fin_from_uut);

    let ack_to_fin_from_uut = Segment::new_empty(Ack, 3002, 1005);
    send_segment_to(&client1_socket, server_addr, &ack_to_fin_from_uut);

    uut_stream2.wait_shutdown_complete();

    println!("{:?}", "done");

    // std::thread::sleep(Duration::from_millis(10000));


    ()
}

////////////////////////////////////////////////////////////////////////////////
// Helper functions
////////////////////////////////////////////////////////////////////////////////

fn recv_segment_from(client_socket: &UdpSocket, uut_addr: SocketAddr) -> Segment {
    let (seg, recv_addr) = recv_segment_with_addr(client_socket);
    assert_eq!(uut_addr, recv_addr);
    seg
}

fn recv_segment_with_addr(client_socket: &UdpSocket) -> (Segment, SocketAddr) {
    let mut buf = [0; 4096];
    let (amt, recv_addr) = client_socket.recv_from(&mut buf).unwrap();
    (Segment::decode(&buf[0..amt]).unwrap(), recv_addr)
}

fn send_segment_to(
    client_socket: &UdpSocket,
    uut_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    client_socket.send_to(&encoded_seq, uut_addr).unwrap();
}
