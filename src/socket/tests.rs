use super::*;

mod mock_timer;

use async_std::future;
use std::io::{Error, ErrorKind};
use std::net::{UdpSocket, *};
use std::time::Duration;

use self::mock_timer::MockTimer;
use crate::segment::Segment;

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

// Test cases needed:
// - Cumulative ack, one byte more and one byte less
//   than the border
// - Send long data that needs two segments
// - shutdown one side at a time
// - shutdown an retransmissions one side at a time
// - close causes segment to be lost
// - one segment is missed, second arrives. i.e. receive side buffering
// - retransmission of something already delivered to the user

// TODO: Put Main/alternative/expcetional in separate files instead
////////////////////////////////////////////////////////////////////////////////
// Main flow test cases
////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_connect_shutdown() {
    let state = setup_connected_uut_client();
    shutdown(state);
}

#[test]
fn test_shutdown_tc_before_uut() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_write(&mut state, b"some data to write");
    main_flow_uut_read(&mut state, b"some data to read");

    main_flow_tc_shutdown(&mut state);
    main_flow_uut_shutdown(&mut state);
    wait_shutdown_complete(state);
}

#[test]
fn test_shutdown_tc_before_uut_write_after_shutdown() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_write(&mut state, b"some data to write");
    main_flow_uut_read(&mut state, b"some data to read");

    main_flow_tc_shutdown(&mut state);

    // tc side has sent FIN. But uut hasn't, so uut can still write
    main_flow_uut_write(&mut state, b"some data");
    main_flow_uut_shutdown(&mut state);

    wait_shutdown_complete(state);
}

#[test]
fn test_shutdown_uut_before_tc_read_after_shutdown() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_write(&mut state, b"some data to write");
    main_flow_uut_read(&mut state, b"some data to read");

    // uut side has sent FIN. But tc hasn't, so tc can still write and uut can
    // read
    main_flow_uut_shutdown(&mut state);
    main_flow_uut_read(&mut state, b"some data");
    main_flow_tc_shutdown(&mut state);

    wait_shutdown_complete(state);
}

#[test]
fn test_close() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_read(&mut state, b"some data");
    main_flow_uut_write(&mut state, b"some data");

    state.uut_stream.take().unwrap().close();
    test_end_check(&mut state);
}

#[test]
fn test_client_read_once() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_read(&mut state, b"some data");

    shutdown(state);
}

#[test]
fn test_client_read_multiple_times() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_read(&mut state, b"first rweouinwrte");
    main_flow_uut_read(&mut state, b"second hfuiasud");
    main_flow_uut_read(&mut state, b"third uifdshufihsiughsyudfghkusfdf");
    main_flow_uut_read(&mut state, b"fourth fuidshfadgaerge");
    main_flow_uut_read(&mut state, b"fifth dhuifghuifdlfoiwejiow");
    main_flow_uut_read(&mut state, b"sixth fdauykfudsfgs");
    main_flow_uut_read(&mut state, b"seventh fsdhsdgfsd");
    main_flow_uut_read(&mut state, b"eighth ijogifdgire");
    main_flow_uut_read(&mut state, b"ninth ertwrw");
    main_flow_uut_read(&mut state, b"tenth uhfsdghsu");

    shutdown(state);
}

#[test]
fn test_client_write_once() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_write(&mut state, b"some data");

    shutdown(state);
}

#[test]
fn test_client_write_multiple_times() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_write(&mut state, b"first agfs");
    main_flow_uut_write(&mut state, b"second gfdhdgfh");
    main_flow_uut_write(&mut state, b"third dfafsdfads");
    main_flow_uut_write(&mut state, b"fourth dfafas");
    main_flow_uut_write(&mut state, b"fifth dfasfasfsdaf");
    main_flow_uut_write(&mut state, b"sixth thythrt");
    main_flow_uut_write(&mut state, b"seventh fdsaref");
    main_flow_uut_write(&mut state, b"eighth dagfsdrgrege");
    main_flow_uut_write(&mut state, b"ninth asfaerger");
    main_flow_uut_write(&mut state, b"tenth trehjk");

    shutdown(state);
}

#[test]
fn test_client_reads_and_writes() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_read(&mut state, b"first");
    main_flow_uut_write(&mut state, b"second");
    main_flow_uut_write(&mut state, b"third");
    main_flow_uut_write(&mut state, b"fourth");
    main_flow_uut_read(&mut state, b"fifth");
    main_flow_uut_read(&mut state, b"sixth");
    main_flow_uut_write(&mut state, b"seventh");

    shutdown(state);
}

////////////////////////////////////////////////////////////////////////////////
// Alternative flow test cases
////////////////////////////////////////////////////////////////////////////////

// TODO: Idea: when all test cases need a state parameter anyway, try
// to chain them together, running one scenario after the other
#[test]
fn test_client_write_retransmit_due_to_timeout() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    main_flow_uut_write(&mut state, b"some initial data");

    let initial_uut_seq_num = state.uut_seq_num;

    // Send data from uut
    state.timer.expect_call_to_sleep();
    let data = b"some data";
    let (len, recv_seg) = uut_write(&mut state, data);
    state.timer.wait_for_call_to_sleep();

    // tc pretends it didn't get data by not sending an ACK. Instead,
    // the timeout expires
    state.timer.trigger_and_expect_new_call();
    state.timer.wait_for_call_to_sleep();

    expect_segment(&recv_seg, &state);

    let ack =
        Segment::new_empty(Ack, state.tc_seq_num, initial_uut_seq_num + len);
    send_segment(&state, &ack);

    shutdown(state);
}

#[test]
fn test_client_write_retransmit_multiple_segments_due_to_timeout() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    main_flow_uut_write(&mut state, b"some initial data");

    let initial_uut_seq_num = state.uut_seq_num;

    // Send data from uut
    state.timer.expect_call_to_sleep();
    let data1 = b"some data";
    let (len1, recv_seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call_to_sleep();

    // Note that the timer was only started for the first write
    let data2 = b"some other data";
    let (len2, recv_seg2) = uut_write(&mut state, data2);

    let data3 = b"some more data";
    let (len3, recv_seg3) = uut_write(&mut state, data3);

    // tc pretends it didn't get data by not sending an ACK. Instead,
    // the timeout expires
    state.timer.trigger_and_expect_new_call();
    state.timer.wait_for_call_to_sleep();

    expect_segment(&recv_seg1, &state);
    expect_segment(&recv_seg2, &state);
    expect_segment(&recv_seg3, &state);

    let ack1 =
        Segment::new_empty(Ack, state.tc_seq_num, initial_uut_seq_num + len1);
    let ack2 = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        initial_uut_seq_num + len1 + len2,
    );
    let ack3 = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        initial_uut_seq_num + len1 + len2 + len3,
    );

    state.timer.expect_call_to_sleep();
    send_segment(&state, &ack1);
    state.timer.wait_for_call_to_sleep();

    state.timer.expect_call_to_sleep();
    send_segment(&state, &ack2);
    state.timer.wait_for_call_to_sleep();

    send_segment(&state, &ack3);

    shutdown(state);
}

#[test]
fn test_client_write_retransmit_due_to_old_ack() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    main_flow_uut_write(&mut state, b"some initial data");

    let initial_uut_seq_num = state.uut_seq_num;

    // Send data1 from uut
    state.timer.expect_call_to_sleep();
    let data1 = b"first data";
    let (len1, recv_seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call_to_sleep();

    // Send data2 from uut
    let data2 = b"second data";
    let (len2, recv_seg2) = uut_write(&mut state, data2);

    // tc pretends that it didn't get data1 by sending ACK (dup ack, fast
    // retransmit) for the original seq_num
    state.timer.expect_call_to_sleep();
    let send_ack0 =
        Segment::new_empty(Ack, state.tc_seq_num, initial_uut_seq_num);
    send_segment(&state, &send_ack0);
    state.timer.wait_for_call_to_sleep();

    // This causes uut to retransmit everything from the acked seq_num to
    // "current"
    expect_segment(&recv_seg1, &state);
    expect_segment(&recv_seg2, &state);

    // Now the tc sends ack for both of them
    state.timer.expect_call_to_sleep();
    let send_ack1 =
        Segment::new_empty(Ack, state.tc_seq_num, initial_uut_seq_num + len1);
    send_segment(&state, &send_ack1);
    state.timer.wait_for_call_to_sleep();

    let send_ack2 = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        initial_uut_seq_num + len1 + len2,
    );
    send_segment(&state, &send_ack2);

    shutdown(state);
}

#[test]
fn test_first_segment_acked_but_not_second() {
    let mut state = setup_connected_uut_client();
    let initial_uut_seq_num = state.uut_seq_num;

    state.timer.expect_call_to_sleep();
    let data1 = b"some data";
    let (len1, _recv_seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call_to_sleep();

    let data2 = b"some other data";
    let (len2, recv_seg2) = uut_write(&mut state, data2);

    let ack1 =
        Segment::new_empty(Ack, state.tc_seq_num, initial_uut_seq_num + len1);

    // TC sends Ack for the first segment, but not the second
    // This causes the timer to be restarted
    state.timer.expect_call_to_sleep();
    send_segment(&state, &ack1);
    state.timer.wait_for_call_to_sleep();
    state.timer.trigger_and_expect_new_call();

    // Only the unacked segment is retransmitted
    expect_segment(&recv_seg2, &state);
    state.timer.wait_for_call_to_sleep();

    let ack2 = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        initial_uut_seq_num + len1 + len2,
    );
    send_segment(&state, &ack2);

    shutdown(state);
}

#[test]
fn test_cumulative_ack() {
    let mut state = setup_connected_uut_client();
    let initial_uut_seq_num = state.uut_seq_num;

    state.timer.expect_call_to_sleep();
    let data1 = b"some data";
    let (len1, _seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call_to_sleep();

    let data2 = b"some other data";
    let (len2, _seg2) = uut_write(&mut state, data2);

    let ack = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        initial_uut_seq_num + len1 + len2,
    );

    send_segment(&state, &ack);

    shutdown(state);
}

#[test]
fn test_multi_segment_write() {
    let mut state = setup_connected_uut_client();

    // Generate some long data to write
    let len = MAXIMUM_SEGMENT_SIZE + 10 + rand::random::<u32>() % 100;
    assert!(len < MAXIMUM_SEGMENT_SIZE * 2);
    let mut data: Vec<u8> = Vec::new();
    for _ in 0..len {
        data.push(rand::random());
    }

    // Send from uut
    state.timer.expect_call_to_sleep();
    let written_len =
        state.uut_stream.as_mut().unwrap().write(&data).unwrap() as u32;
    assert_eq!(len, written_len);
    state.timer.wait_for_call_to_sleep();

    // Receive the first segment
    let exp_seg1 = Segment::new(
        Ack,
        state.uut_seq_num,
        state.tc_seq_num,
        &data[0..MAXIMUM_SEGMENT_SIZE as usize],
    );
    expect_segment(&exp_seg1, &state);

    // Receive the second segment
    let exp_seg2 = Segment::new(
        Ack,
        state.uut_seq_num + MAXIMUM_SEGMENT_SIZE,
        state.tc_seq_num,
        &data[MAXIMUM_SEGMENT_SIZE as usize..],
    );
    expect_segment(&exp_seg2, &state);

    state.uut_seq_num += len;

    let ack = Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num);

    send_segment(&state, &ack);

    shutdown(state);
}

#[test]
fn test_same_segment_carries_data_and_acks() {
    let mut state = setup_connected_uut_client();

    // Send some data from uut
    state.timer.expect_call_to_sleep();
    let data_from_uut = b"some data";
    uut_write(&mut state, data_from_uut);
    state.timer.wait_for_call_to_sleep();

    // Then send some data from tc, without first sending a separate ACK
    main_flow_uut_read(&mut state, b"some other data");

    shutdown(state);
}

#[test]
fn test_out_of_order_segment() {
    let mut state = setup_connected_uut_client();

    let data1 = b"some data";
    let len1 = data1.len() as u32;
    let seg1 = Segment::new(Ack, state.tc_seq_num, state.uut_seq_num, data1);

    let data2 = b"some other data";
    let len2 = data2.len() as u32;
    let seg2 =
        Segment::new(Ack, state.tc_seq_num + len1, state.uut_seq_num, data2);

    send_segment(&state, &seg2);

    // ACK for the data before seg1 received
    let exp_ack = Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);
    expect_segment(&exp_ack, &state);

    // TODO: Check that uut read fails

    send_segment(&state, &seg1);

    // But when the gap is filled, the ACK acks everything sent
    let exp_ack_all = Segment::new_empty(
        Ack,
        state.uut_seq_num,
        state.tc_seq_num + len1 + len2,
    );
    expect_segment(&exp_ack_all, &state);

    state.tc_seq_num += len1 + len2;

    let read_data1 = read_uut_stream_once(&mut state);
    assert_eq!(data1.to_vec(), read_data1);
    let read_data2 = read_uut_stream_once(&mut state);
    assert_eq!(data2.to_vec(), read_data2);

    shutdown(state);
}

////////////////////////////////////////////////////////////////////////////////
// Helper functions
////////////////////////////////////////////////////////////////////////////////

struct State {
    tc_socket: UdpSocket,
    uut_stream: Option<Stream>,
    uut_addr: SocketAddr,
    tc_seq_num: u32,
    uut_seq_num: u32,
    timer: Arc<MockTimer>,
}

fn test_end_check(state: &mut State) {
    // To catch late calls to the timer
    std::thread::sleep(Duration::from_millis(1));
    recv_check_no_data(&mut state.tc_socket);
    assert!(state.uut_stream.is_none());
    state.timer.test_end_check();
    // TODO: Check that no data can be read
}

fn setup_connected_uut_client() -> State {
    let tc_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tc_socket = UdpSocket::bind(tc_addr).unwrap();
    tc_socket
        .set_read_timeout(Some(Duration::from_millis(5)))
        .unwrap();

    uut_connect(tc_socket)
}

fn uut_connect(tc_socket: UdpSocket) -> State {
    // Connect
    let timer = Arc::new(MockTimer::new());
    let timer_cloned = Arc::clone(&timer);
    let tc_addr = tc_socket.local_addr().unwrap();
    let join_handle = thread::Builder::new()
        .name("connect client".to_string())
        .spawn(move || {
            Stream::connect_custom_timer(timer_cloned, tc_addr).unwrap()
        })
        .unwrap();

    // Receive SYN
    let (syn, uut_addr) = recv_segment_with_addr(&tc_socket);
    assert_eq!(Syn, syn.kind());

    // Send SYN-ACK
    let mut tc_seq_num = rand::random();
    let uut_seq_num = syn.seq_num() + 1;
    let syn_ack = Segment::new_empty(SynAck, tc_seq_num, uut_seq_num);
    send_segment_to(&tc_socket, uut_addr, &syn_ack);

    // Receive ACK
    let ack = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(Ack, ack.kind());
    tc_seq_num += 1;
    assert_eq!(tc_seq_num, ack.ack_num());

    let uut_stream = join_handle.join().unwrap();

    State {
        tc_socket,
        uut_stream: Some(uut_stream),
        uut_addr,
        tc_seq_num,
        uut_seq_num,
        timer,
    }
}

// Disconnect should send FIN, not accept more writes
// When receive FIN, send FIN, then same as above
// When all data has been read, return 0 length
// close stops the process
// If close returns true, it means buffer emppty, include FIN
// so all data was received
// Perhaps also add a poll function to check if closing state
fn shutdown(mut state: State) {
    main_flow_uut_shutdown(&mut state);
    main_flow_tc_shutdown(&mut state);
    wait_shutdown_complete(state);
}

fn main_flow_uut_shutdown(state: &mut State) {
    // To make sure the buffer is empty so that a new timer call
    // will be made
    thread::sleep(Duration::from_millis(1));

    state.timer.expect_call_to_sleep();
    state.uut_stream.as_mut().unwrap().shutdown();
    state.timer.wait_for_call_to_sleep();

    // Write fails
    let write_result = state.uut_stream.as_mut().unwrap().write(b"some data");
    assert_eq!(write_result.unwrap_err().kind(), ErrorKind::NotConnected);

    // uut sends FIN
    let exp_fin = Segment::new_empty(Fin, state.uut_seq_num, state.tc_seq_num);
    expect_segment(&exp_fin, &state);

    // tc sends ACK to the FIN
    let ack_to_fin =
        Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num + 1);
    send_segment(&state, &ack_to_fin);
    state.uut_seq_num += 1;
}

fn main_flow_tc_shutdown(state: &mut State) {
    // TODO: Check that read returns 0
    // tc sends FIN
    let send_seg = Segment::new_empty(Fin, state.tc_seq_num, state.uut_seq_num);
    send_segment(&state, &send_seg);
    state.tc_seq_num += 1;

    // uut sends ACK to the FIN
    let exp_ack_to_fin =
        Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);
    expect_segment(&exp_ack_to_fin, &state);
}

fn wait_shutdown_complete(mut state: State) {
    state.uut_stream.take().unwrap().wait_shutdown_complete();
    test_end_check(&mut state);
}

fn main_flow_uut_read(state: &mut State, data: &[u8]) {
    // Send from the tc
    let send_seg = Segment::new(Ack, state.tc_seq_num, state.uut_seq_num, data);
    send_segment(&state, &send_seg);
    state.tc_seq_num += data.len() as u32;

    // Recv ACK from the uut
    let exp_ack = Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);
    expect_segment(&exp_ack, &state);

    // Check that the uut received the correct data
    let read_data = read_uut_stream_once(state);
    assert_eq!(data, read_data);
}

fn read_uut_stream_once(state: &mut State) -> Vec<u8> {
    let mut buf = [0; 4096];
    let amt = state.uut_stream.as_mut().unwrap().read(&mut buf).unwrap();
    buf[0..amt].to_vec()
}

fn main_flow_uut_write(state: &mut State, data: &[u8]) {
    // Send from the uut
    state.timer.expect_call_to_sleep();
    uut_write(state, data);
    state.timer.wait_for_call_to_sleep();

    // Send ack from the tc
    let send_seg = Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num);
    send_segment(&state, &send_seg);

    recv_check_no_data(&state.tc_socket);
}

fn uut_write(state: &mut State, data: &[u8]) -> (u32, Segment) {
    // Send from the uut
    let written_len = state.uut_stream.as_mut().unwrap().write(&data).unwrap();
    let len = data.len();
    assert_eq!(len, written_len);

    // Recv from the tc
    let exp_seg = Segment::new(Ack, state.uut_seq_num, state.tc_seq_num, &data);
    expect_segment(&exp_seg, &state);
    state.uut_seq_num += len as u32;

    (len as u32, exp_seg)
}

fn expect_segment(exp_seg: &Segment, state: &State) {
    let recv_seg = recv_segment(state);
    assert_eq!(exp_seg, &recv_seg);
}

fn recv_segment(state: &State) -> Segment {
    recv_segment_from(&state.tc_socket, state.uut_addr)
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

fn recv_check_no_data(tc_socket: &UdpSocket) {
    let mut buf = [0; 4096];
    match tc_socket.recv(&mut buf) {
        Err(err) => assert_eq!(ErrorKind::WouldBlock, err.kind()),
        other => {
            panic!("Didn't get WouldBlock, instead got: {:?}", other);
        }
    };
}

fn send_segment(state: &State, segment: &Segment) {
    send_segment_to(&state.tc_socket, state.uut_addr, segment);
}

fn send_segment_to(
    tc_socket: &UdpSocket,
    uut_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    tc_socket.send_to(&encoded_seq, uut_addr).unwrap();
}

// Possibile "white box" things, or "test window" things
// - Number of discarded received segments
// - Buffer size
