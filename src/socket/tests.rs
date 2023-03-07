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
// - shutdown an retransmissions one side at a time
// - close causes segment to be lost
// - buf too small, i.e. need to have a field in ClientStream for storing
//   potentially data received on last channel read.

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
    let data = random_data_of_length(len);

    // Send from uut
    state.timer.expect_call_to_sleep();
    let written_len = uut_stream(&mut state).write(&data).unwrap() as u32;
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

fn random_data_of_length(length: u32) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::new();
    for _ in 0..length {
        data.push(rand::random());
    }
    data
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

    expect_read(&[data1, data2], &mut state);

    shutdown(state);
}

#[test]
fn test_multiple_out_of_order_segments() {
    let mut state = setup_connected_uut_client();

    let mut segments = Vec::new();
    let len = 10;

    for segment_index in 0..9 {
        let data = random_data_of_length(len);
        let accumlative_len = segment_index * len;
        let seg = Segment::new(
            Ack,
            state.tc_seq_num + accumlative_len,
            state.uut_seq_num,
            &data,
        );
        segments.push(seg);
    }

    let ack_base = Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);

    // TODO: Make arg order consistent
    // |  2      |
    send_segment(&state, &segments[2]);
    expect_segment(&ack_base, &state);
    expect_read_no_data(&mut state);

    // |  23     |
    send_segment(&state, &segments[3]);
    expect_segment(&ack_base, &state);
    expect_read_no_data(&mut state);

    // |  23  6  |
    send_segment(&state, &segments[6]);
    expect_segment(&ack_base, &state);
    expect_read_no_data(&mut state);

    // |  23  6 8|
    send_segment(&state, &segments[8]);
    expect_segment(&ack_base, &state);
    expect_read_no_data(&mut state);

    // |0 23  6 8|
    send_segment(&state, &segments[0]);
    let ack0 =
        Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num + len * 1);
    expect_segment(&ack0, &state);
    expect_read_data_of_segments(&segments[0..=0], &mut state);

    // |0123  6 8|
    send_segment(&state, &segments[1]);
    let ack3 =
        Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num + len * 4);
    expect_segment(&ack3, &state);
    expect_read_data_of_segments(&segments[1..=3], &mut state);

    // |0123 56 8|
    send_segment(&state, &segments[5]);
    expect_segment(&ack3, &state);
    expect_read_no_data(&mut state);

    // |0123456 8|
    send_segment(&state, &segments[4]);
    let ack6 =
        Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num + len * 7);
    expect_segment(&ack6, &state);
    expect_read_data_of_segments(&segments[4..=6], &mut state);

    // |012345678|
    send_segment(&state, &segments[7]);
    let ack8 =
        Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num + len * 9);
    expect_segment(&ack8, &state);
    expect_read_data_of_segments(&segments[7..=8], &mut state);

    state.tc_seq_num += len * 9;

    shutdown(state);
}

#[test]
fn test_out_of_order_receive_buffer_full() {
    let mut state = setup_connected_uut_client();

    let mut segments = Vec::new();
    let num_segments = (MAXIMUM_RECV_BUFFER_SIZE + 1) as u32;
    let len = 20;

    for segment_index in 0..num_segments as u32 {
        let data = random_data_of_length(len);
        let accumlative_len = segment_index * len;
        let seg = Segment::new(
            Ack,
            state.tc_seq_num + accumlative_len,
            state.uut_seq_num,
            &data,
        );
        segments.push(seg);
    }

    let ack_base = Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);

    // Send all segments except the first
    for segment_index in 1..num_segments {
        send_segment(&state, &segments[segment_index as usize]);
        expect_segment(&ack_base, &state);
    }

    // Now send the first, and get an ack for all segments except the last
    send_segment(&state, &segments[0]);
    let offset_second_to_last = (num_segments - 1) * len;
    let ack_all_except_last = Segment::new_empty(
        Ack,
        state.uut_seq_num,
        state.tc_seq_num + offset_second_to_last,
    );
    expect_segment(&ack_all_except_last, &state);

    // Now all data except the last can be read
    expect_read_data_of_segments(
        &segments[0..(num_segments - 1) as usize],
        &mut state,
    );

    state.tc_seq_num += offset_second_to_last;

    shutdown(state);
}

#[test]
fn test_retransmission_of_data() {
    let mut state = setup_connected_uut_client();

    let (sent_seg, received_ack) =
        main_flow_uut_read(&mut state, b"some data to read");
    send_segment(&mut state, &sent_seg);
    expect_segment(&received_ack, &state);

    main_flow_uut_read(&mut state, b"some other data");
    main_flow_uut_write(&mut state, b"some data to write");

    shutdown(state);
}

#[test]
fn test_retransmission_of_fin() {
    let mut state = setup_connected_uut_client();

    main_flow_uut_read(&mut state, b"some data");
    main_flow_uut_write(&mut state, b"some data to write");

    let (sent_fin, received_ack) = main_flow_tc_shutdown(&mut state);
    send_segment(&mut state, &sent_fin);
    expect_segment(&received_ack, &state);

    main_flow_uut_write(&mut state, b"some other data to write");

    main_flow_uut_shutdown(&mut state);
    wait_shutdown_complete(state);
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
    // TODO: Check that all buffers empty
}

fn setup_connected_uut_client() -> State {
    let tc_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tc_socket = UdpSocket::bind(tc_addr).unwrap();
    tc_socket
        .set_read_timeout(Some(Duration::from_millis(2)))
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

    let mut uut_stream = join_handle.join().unwrap();
    uut_stream.set_read_timeout(Some(Duration::from_millis(2)));

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
    uut_stream(state).shutdown();
    state.timer.wait_for_call_to_sleep();

    // Since FIN has been sent, write fails
    let write_result = uut_stream(state).write(b"some data");
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

fn main_flow_tc_shutdown(state: &mut State) -> (Segment, Segment) {
    // tc sends FIN
    let send_seg = Segment::new_empty(Fin, state.tc_seq_num, state.uut_seq_num);
    send_segment(&state, &send_seg);
    state.tc_seq_num += 1;

    // uut sends ACK to the FIN
    let exp_ack_to_fin =
        Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);
    expect_segment(&exp_ack_to_fin, &state);

    // Since FIN has been received, 0 data is returned
    let mut buf = [0; 123];
    let buf_before = buf.clone();
    let read_len = uut_stream(state).read(&mut buf).unwrap();
    assert_eq!(0, read_len);
    assert_eq!(buf_before, buf);

    (send_seg, exp_ack_to_fin)
}

fn wait_shutdown_complete(mut state: State) {
    state.uut_stream.take().unwrap().wait_shutdown_complete();
    test_end_check(&mut state);
}

fn main_flow_uut_read(state: &mut State, data: &[u8]) -> (Segment, Segment) {
    // Send from the tc
    let send_seg = Segment::new(Ack, state.tc_seq_num, state.uut_seq_num, data);
    send_segment(&state, &send_seg);
    state.tc_seq_num += data.len() as u32;

    // Recv ACK from the uut
    let exp_ack = Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);
    expect_segment(&exp_ack, &state);

    // Check that the uut received the correct data
    expect_read(&[data], state);

    (send_seg, exp_ack)
}

fn expect_read_data_of_segments<'a, I>(segments: I, state: &mut State)
where
    I: IntoIterator<Item = &'a Segment>,
{
    let exp_datas: Vec<_> =
        segments.into_iter().map(|seg| seg.data()).collect();
    expect_read(&exp_datas, state);
}

fn expect_read(exp_datas: &[&[u8]], state: &mut State) {
    let mut all_exp_data = Vec::new();

    for exp_data in exp_datas {
        all_exp_data.extend_from_slice(exp_data);
    }

    let mut read_data = vec![0; all_exp_data.len()];
    assert_ne!(all_exp_data, read_data);
    uut_stream(state).read_exact(&mut read_data).unwrap();
    assert_eq!(all_exp_data, read_data);
    expect_read_no_data(state);
}

fn uut_stream(state: &mut State) -> &mut Stream {
    state.uut_stream.as_mut().unwrap()
}

fn expect_read_no_data(state: &mut State) {
    let mut buf = [0; 1];
    let res = uut_stream(state).read(&mut buf);
    assert_eq!(ErrorKind::WouldBlock, res.unwrap_err().kind());
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
    let written_len = uut_stream(state).write(&data).unwrap();
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
