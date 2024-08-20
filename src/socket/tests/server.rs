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
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut listener = Listener::bind(initial_addr).unwrap();
    let server_addr = listener.local_addr().unwrap();

    //////////////////////////////////////////////////////////////////
    // Connect
    //////////////////////////////////////////////////////////////////

    let connect_client_thread = thread::spawn(move || {
        let client_socket = UdpSocket::bind(initial_addr).unwrap();
        let tc_addr = client_socket.local_addr().unwrap();
        assert_ne!(tc_addr, server_addr);

        UdpSocket::connect(&client_socket, server_addr).unwrap();

        // Send SYN
        let syn = Segment::new_empty(Syn, 2000, 0);
        send_segment_to(&client_socket, server_addr, &syn);

        // Receive SYN-ACK
        let syn_ack = recv_segment_from(&client_socket, server_addr);

        // TODO: 1000 is hard coded. Need to change
        let exp_syn_ack = Segment::new_empty(SynAck, 1000, 2001);
        assert_eq!(exp_syn_ack, syn_ack);

        // Send ACK
        let ack = Segment::new_empty(Ack, 2001, 1001);
        send_segment_to(&client_socket, server_addr, &ack);

        // println!("{:?}", "tc: connect done");

        client_socket
    });

    let (mut uut_stream, client_addr) = listener.accept().unwrap();
    let client_socket = connect_client_thread.join().unwrap();
    assert_eq!(client_addr, client_socket.local_addr().unwrap());

    //////////////////////////////////////////////////////////////////
    // Write #1
    //////////////////////////////////////////////////////////////////

    uut_stream.write(b"hello").unwrap();

    let exp_seg_write1 = Segment::new(Ack, 1001, 2001, b"hello");
    let seg_write1 = recv_segment_from(&client_socket, server_addr);
    assert_eq!(exp_seg_write1, seg_write1);

    let ack_seg_write1 = Segment::new_empty(Ack, 2001, 1006);
    send_segment_to(&client_socket, server_addr, &ack_seg_write1);

    // println!("write1 done");

    //////////////////////////////////////////////////////////////////
    // Write #2
    //////////////////////////////////////////////////////////////////

    uut_stream.write(b"more").unwrap();

    let exp_seg_write2 = Segment::new(Ack, 1006, 2001, b"more");
    let seg_write2 = recv_segment_from(&client_socket, server_addr);
    assert_eq!(exp_seg_write2, seg_write2);

    let ack_seg_write2 = Segment::new_empty(Ack, 2001, 1010);
    send_segment_to(&client_socket, server_addr, &ack_seg_write2);

    // println!("write2 done");

    //////////////////////////////////////////////////////////////////
    // Read
    //////////////////////////////////////////////////////////////////

    let seg_read1 = Segment::new(Ack, 2001, 1010, b"From test case");
    send_segment_to(&client_socket, server_addr, &seg_read1);

    let exp_ack_read1 = Segment::new_empty(Ack, 1010, 2015);
    let ack_read1 = recv_segment_from(&client_socket, server_addr);
    assert_eq!(exp_ack_read1, ack_read1);

    // println!("read done");

    //////////////////////////////////////////////////////////////////
    // Shutdown from uut
    //////////////////////////////////////////////////////////////////

    uut_stream.shutdown();

    let exp_fin = Segment::new_empty(Fin, 1010, 2015);
    let fin_from_uut = recv_segment_from(&client_socket, server_addr);
    assert_eq!(exp_fin, fin_from_uut);

    let ack_to_fin_from_uut = Segment::new_empty(Ack, 2015, 1011);
    send_segment_to(&client_socket, server_addr, &ack_to_fin_from_uut);

    //////////////////////////////////////////////////////////////////
    // Shutdown from tc
    //////////////////////////////////////////////////////////////////

    let fin_from_tc = Segment::new_empty(Fin, 2015, 1011);
    send_segment_to(&client_socket, server_addr, &fin_from_tc);

    let exp_ack_to_fin_from_tc = Segment::new_empty(Ack, 1011, 2016);
    let ack_to_fin_from_tc = recv_segment_from(&client_socket, server_addr);
    assert_eq!(exp_ack_to_fin_from_tc, ack_to_fin_from_tc);

    uut_stream.wait_shutdown_complete();

    // println!("{:?}", "done");

    listener.shutdown_all();
    listener.wait_shutdown_complete();
}

#[test]
fn mf_explicit_sequence_numbers_two_clients() {
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut listener = Listener::bind(initial_addr).unwrap();
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

        // println!("{:?}", "tc: connect1 done");

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

        // println!("{:?}", "tc: connect2 done");

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

    // println!("write1 client1 done");

    //////////////////////////////////////////////////////////////////
    // Write #1 - client 2
    //////////////////////////////////////////////////////////////////

    uut_stream2.write(b"hej").unwrap();

    let exp_seg_write1_client2 = Segment::new(Ack, 1001, 3001, b"hej");
    let seg_write1_client2 = recv_segment_from(&client2_socket, server_addr);
    assert_eq!(exp_seg_write1_client2, seg_write1_client2);

    let ack_seg_write1 = Segment::new_empty(Ack, 2001, 1004);
    send_segment_to(&client1_socket, server_addr, &ack_seg_write1);

    // println!("write1 client2 done");

    //////////////////////////////////////////////////////////////////
    // Write #2
    //////////////////////////////////////////////////////////////////

    uut_stream1.write(b"more").unwrap();

    let exp_seg_write2 = Segment::new(Ack, 1006, 2001, b"more");
    let seg_write2 = recv_segment_from(&client1_socket, server_addr);
    assert_eq!(exp_seg_write2, seg_write2);

    let ack_seg_write2 = Segment::new_empty(Ack, 2001, 1010);
    send_segment_to(&client1_socket, server_addr, &ack_seg_write2);

    // println!("write2 done");

    //////////////////////////////////////////////////////////////////
    // Read
    //////////////////////////////////////////////////////////////////

    let seg_read1 = Segment::new(Ack, 2001, 1010, b"From test case");
    send_segment_to(&client1_socket, server_addr, &seg_read1);

    let exp_ack_read1 = Segment::new_empty(Ack, 1010, 2015);
    let ack_read1 = recv_segment_from(&client1_socket, server_addr);
    assert_eq!(exp_ack_read1, ack_read1);

    // println!("read done");

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

    // println!("{:?}", "done");

    listener.shutdown_all();
    listener.wait_shutdown_complete();
}

////////////////////////////////////////////////////////////////////////////////
// Read/write test cases
////////////////////////////////////////////////////////////////////////////////

#[test]
fn mf_reads_and_writes() {
    let listener_state = uut_listen();
    let mut stream_state = uut_accept(&listener_state);

    uut_write_with_tc_ack(&mut stream_state, b"some data");
    uut_read(&mut stream_state, b"some data to read");
    uut_write_with_tc_ack(&mut stream_state, b"other data this time");
    uut_read(&mut stream_state, b"some data to read");
}

#[test]
fn mf_reads_and_writes_multiple_clients() {
    let listener_state = uut_listen();
    // TODO: Interleaved accepts/reads/writes in other TC
    let mut stream_state1 = uut_accept(&listener_state);
    let mut stream_state2 = uut_accept(&listener_state);
    let mut stream_state3 = uut_accept(&listener_state);

    uut_write_with_tc_ack(&mut stream_state1, b"one");
    uut_write_with_tc_ack(&mut stream_state2, b"two");
    uut_write_with_tc_ack(&mut stream_state3, b"three");

    uut_read(&mut stream_state1, b"ett");
    uut_read(&mut stream_state2, b"tva");
    uut_read(&mut stream_state3, b"tre");

    uut_write_with_tc_ack(&mut stream_state3, b"hello");
    uut_write_with_tc_ack(&mut stream_state2, b"hej spam spam spam");
    uut_read(&mut stream_state1, b"data to read");
}

////////////////////////////////////////////////////////////////////////////////
// Helper functions
////////////////////////////////////////////////////////////////////////////////

struct ListenerState {
    listener: Listener,
    uut_addr: SocketAddr,
}

fn uut_listen() -> ListenerState {
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = Listener::bind(initial_addr).unwrap();
    let uut_addr = listener.local_addr().unwrap();

    ListenerState { listener, uut_addr }
}

struct StreamState {
    tc_socket: UdpSocket,
    uut_stream: Option<Stream>,
    uut_addr: SocketAddr,
    send_next: u32,
    receive_next: u32,
}

fn uut_accept(listener_state: &ListenerState) -> StreamState {
    let initial_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let uut_addr = listener_state.uut_addr;

    let connect_client_thread = thread::spawn(move || {
        let client_socket = UdpSocket::bind(initial_addr).unwrap();
        let client_addr = client_socket.local_addr().unwrap();
        assert_ne!(client_addr, uut_addr);

        client_socket
            .set_read_timeout(Some(Duration::from_millis(2)))
            .unwrap();

        UdpSocket::connect(&client_socket, uut_addr).unwrap();

        // For seq num, always from UUT's POV
        let mut receive_next = rand::random();

        // Send SYN
        let syn = Segment::new_empty(Syn, receive_next, 0);
        send_segment_to(&client_socket, uut_addr, &syn);
        receive_next += 1;

        // Receive SYN-ACK
        let syn_ack = recv_segment_from(&client_socket, uut_addr);
        assert_eq!(SynAck, syn_ack.kind());
        assert_eq!(receive_next, syn_ack.ack_num());
        assert_eq!(b"", syn_ack.data());
        let send_next = syn_ack.seq_num() + 1;

        // Send ACK
        let ack = Segment::new_empty(Ack, receive_next, send_next);
        send_segment_to(&client_socket, uut_addr, &ack);

        (client_socket, send_next, receive_next)
    });

    let (uut_stream, client_addr) = listener_state.listener.accept().unwrap();
    let (client_socket, send_next, receive_next) =
        connect_client_thread.join().unwrap();
    assert_eq!(client_addr, client_socket.local_addr().unwrap());

    StreamState {
        tc_socket: client_socket,
        uut_stream: Some(uut_stream),
        uut_addr: listener_state.uut_addr,
        send_next,
        receive_next,
    }
}

fn uut_write_with_tc_ack(stream_state: &mut StreamState, data: &[u8]) {
    // Send from the uut
    let written_len = uut_stream(stream_state).write(&data).unwrap();
    let len = data.len();
    assert_eq!(len, written_len);

    // Recv from the tc
    let exp_seg = Segment::new(
        Ack,
        stream_state.send_next,
        stream_state.receive_next,
        &data,
    );
    recv__expect_segment(&stream_state, &exp_seg);
    stream_state.send_next += len as u32;

    // Send ack from the tc
    let send_ack = Segment::new_empty(
        Ack,
        stream_state.receive_next,
        stream_state.send_next,
    );
    send_segment(&stream_state, &send_ack);
}

fn uut_read(stream_state: &mut StreamState, data: &[u8]) {
    // Send from the tc
    let send_seg = Segment::new(
        Ack,
        stream_state.receive_next,
        stream_state.send_next,
        data,
    );
    send_segment(&stream_state, &send_seg);
    stream_state.receive_next += data.len() as u32;

    // Recv ACK from the uut
    let exp_ack = Segment::new_empty(
        Ack,
        stream_state.send_next,
        stream_state.receive_next,
    );
    recv__expect_segment(&stream_state, &exp_ack);
}

fn read__expect_data(state: &mut StreamState, exp_data: &[u8]) {
    let mut read_data = vec![0; exp_data.len()];
    assert_ne!(exp_data, read_data);
    uut_stream(state).read_exact(&mut read_data).unwrap();
    assert_eq!(exp_data, read_data);
}

fn uut_stream(stream_state: &mut StreamState) -> &mut Stream {
    stream_state.uut_stream.as_mut().unwrap()
}

fn recv__expect_segment(stream_state: &StreamState, exp_seg: &Segment) {
    let recv_seg = recv_segment(stream_state);
    assert_eq!(exp_seg, &recv_seg);
}

fn recv_segment(stream_state: &StreamState) -> Segment {
    recv_segment_from(&stream_state.tc_socket, stream_state.uut_addr)
}

fn recv_segment_from(
    client_socket: &UdpSocket,
    uut_addr: SocketAddr,
) -> Segment {
    let (seg, recv_addr) = recv_segment_with_addr(client_socket);
    assert_eq!(uut_addr, recv_addr);
    seg
}

fn recv_segment_with_addr(client_socket: &UdpSocket) -> (Segment, SocketAddr) {
    let mut buf = [0; 4096];
    let (amt, recv_addr) = client_socket.recv_from(&mut buf).unwrap();
    (Segment::decode(&buf[0..amt]).unwrap(), recv_addr)
}

fn send_segment(stream_state: &StreamState, segment: &Segment) {
    send_segment_to(&stream_state.tc_socket, stream_state.uut_addr, segment);
}

fn send_segment_to(
    client_socket: &UdpSocket,
    uut_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    client_socket.send_to(&encoded_seq, uut_addr).unwrap();
}
