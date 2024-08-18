#![allow(non_snake_case)]

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
// - ef: Cumulative ack, one byte more and one byte less
//   than the border
// - ef: close causes segment to be lost
// - mf: single byte read and written
// - af: FIN is sent, but the segment before was lost. I.e. out of order with
//       FIN

// All test cases manage sequence numbers semi-automatically. So verify them
// explicitely here.
#[test]
fn mf_explicit_sequence_numbers() {
    let tc_addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let tc_socket = UdpSocket::bind(tc_addr).unwrap();
    tc_socket
        .set_read_timeout(Some(Duration::from_millis(2)))
        .unwrap();

    //////////////////////////////////////////////////////////////////
    // Connect
    //////////////////////////////////////////////////////////////////

    let timer = Arc::new(MockTimer::new());
    let timer_cloned = Arc::clone(&timer);
    let tc_addr = tc_socket.local_addr().unwrap();
    timer.expect_stop();
    let join_handle = thread::Builder::new()
        .name("connect client".to_string())
        .spawn(move || {
            Stream::connect_custom(timer_cloned, tc_addr, 1000).unwrap()
        })
        .unwrap();

    // Receive SYN
    let (syn, uut_addr) = recv_segment_with_addr(&tc_socket);
    let exp_syn = Segment::new_empty(Syn, 1000, 0);
    assert_eq!(exp_syn, syn);

    // Send SYN-ACK
    let syn_ack = Segment::new_empty(SynAck, 2000, 1001);
    send_segment_to(&tc_socket, uut_addr, &syn_ack);

    // Receive ACK
    let ack = recv_segment_from(&tc_socket, uut_addr);
    let exp_ack = Segment::new_empty(Ack, 1001, 2001);
    assert_eq!(exp_ack, ack);

    let mut uut_stream = join_handle.join().unwrap();
    timer.wait_for_call();
    uut_stream.set_read_timeout(Some(Duration::from_millis(2)));

    //////////////////////////////////////////////////////////////////
    // Write #1
    //////////////////////////////////////////////////////////////////

    timer.expect_start();
    uut_stream.write(b"hello").unwrap();
    timer.wait_for_call();

    let exp_seg_write1 = Segment::new(Ack, 1001, 2001, b"hello");
    let seg_write1 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_seg_write1, seg_write1);

    timer.expect_stop();
    let ack_seg_write1 = Segment::new_empty(Ack, 2001, 1006);
    send_segment_to(&tc_socket, uut_addr, &ack_seg_write1);
    timer.wait_for_call();

    //////////////////////////////////////////////////////////////////
    // Write #2
    //////////////////////////////////////////////////////////////////

    timer.expect_start();
    uut_stream.write(b"more").unwrap();
    timer.wait_for_call();

    let exp_seg_write2 = Segment::new(Ack, 1006, 2001, b"more");
    let seg_write2 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_seg_write2, seg_write2);

    timer.expect_stop();
    let ack_seg_write2 = Segment::new_empty(Ack, 2001, 1010);
    send_segment_to(&tc_socket, uut_addr, &ack_seg_write2);
    timer.wait_for_call();

    //////////////////////////////////////////////////////////////////
    // Read
    //////////////////////////////////////////////////////////////////

    let seg_read1 = Segment::new(Ack, 2001, 1010, b"From test case");
    send_segment_to(&tc_socket, uut_addr, &seg_read1);

    let exp_ack_read1 = Segment::new_empty(Ack, 1010, 2015);
    let ack_read1 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_ack_read1, ack_read1);

    //////////////////////////////////////////////////////////////////
    // Write one byte
    //////////////////////////////////////////////////////////////////

    timer.expect_start();
    uut_stream.write(b"x").unwrap();
    timer.wait_for_call();

    let exp_seg_write3 = Segment::new(Ack, 1010, 2015, b"x");
    let seg_write3 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_seg_write3, seg_write3);

    timer.expect_stop();
    let ack_seg_write3 = Segment::new_empty(Ack, 2015, 1011);
    send_segment_to(&tc_socket, uut_addr, &ack_seg_write3);
    timer.wait_for_call();

    //////////////////////////////////////////////////////////////////
    // Read one byte
    //////////////////////////////////////////////////////////////////

    let seg_read2 = Segment::new(Ack, 2015, 1011, b"y");
    send_segment_to(&tc_socket, uut_addr, &seg_read2);

    let exp_ack_read2 = Segment::new_empty(Ack, 1011, 2016);
    let ack_read2 = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_ack_read2, ack_read2);

    //////////////////////////////////////////////////////////////////
    // Shutdown from uut
    //////////////////////////////////////////////////////////////////

    timer.expect_start();
    uut_stream.shutdown();
    timer.wait_for_call();

    let exp_fin = Segment::new_empty(Fin, 1011, 2016);
    let fin_from_uut = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_fin, fin_from_uut);

    timer.expect_stop();
    let ack_to_fin_from_uut = Segment::new_empty(Ack, 2016, 1012);
    send_segment_to(&tc_socket, uut_addr, &ack_to_fin_from_uut);
    timer.wait_for_call();

    //////////////////////////////////////////////////////////////////
    // Shutdown from tc
    //////////////////////////////////////////////////////////////////

    let fin_from_tc = Segment::new_empty(Fin, 2016, 1012);
    send_segment_to(&tc_socket, uut_addr, &fin_from_tc);

    let exp_ack_to_fin_from_tc = Segment::new_empty(Ack, 1012, 2017);
    let ack_to_fin_from_tc = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(exp_ack_to_fin_from_tc, ack_to_fin_from_tc);

    uut_stream.wait_shutdown_complete();
}

////////////////////////////////////////////////////////////////////////////////
// Connect test cases
////////////////////////////////////////////////////////////////////////////////

#[test]
fn mf_connect() {
    setup_connected_uut_client();
}

////////////////////////////////////////////////////////////////////////////////
// Shutdown/close test cases
////////////////////////////////////////////////////////////////////////////////

#[test]
fn mf_shutdown_uut_before_tc() {
    let mut state = setup_connected_uut_client();

    uut_shutdown_with_tc_ack__tc_still_connected(&mut state);
    tc_shutdown(&mut state);
    wait_shutdown_complete(state);
}

#[test]
fn mf_shutdown_uut_before_tc__read_after_shutdown() {
    let mut state = setup_connected_uut_client();

    uut_shutdown_with_tc_ack__tc_still_connected(&mut state);
    // uut side has sent FIN. But tc hasn't, so tc can still write and uut can
    // read
    uut_read(&mut state, b"some data");
    tc_shutdown(&mut state);

    wait_shutdown_complete(state);
}

#[test]
fn mf_shutdown_uut_before_tc__write_after_shutdown_fails() {
    let mut state = setup_connected_uut_client();

    uut_shutdown_with_tc_ack__tc_still_connected(&mut state);

    // Since FIN has been sent, write fails
    let write_result = uut_stream(&mut state).write(b"some data");
    assert_eq!(write_result.unwrap_err().kind(), ErrorKind::NotConnected);

    tc_shutdown(&mut state);

    wait_shutdown_complete(state);
}

#[test]
fn mf_shutdown_tc_before_uut() {
    let mut state = setup_connected_uut_client();

    tc_shutdown(&mut state);
    uut_shutdown_with_tc_ack__tc_already_shutdown(&mut state);
    wait_shutdown_complete(state);
}

#[test]
fn mf_shutdown_tc_before_uut__write_after_shutdown() {
    let mut state = setup_connected_uut_client();

    tc_shutdown(&mut state);

    // tc side has sent FIN. But uut hasn't, so uut can still write
    uut_write_with_tc_ack(&mut state, b"some data");
    uut_shutdown_with_tc_ack__tc_already_shutdown(&mut state);

    wait_shutdown_complete(state);
}

#[test]
fn mf_simultaneous_shutdown() {
    let mut state = setup_connected_uut_client();

    // uut sends FIN
    state.timer.expect_start();
    uut_stream(&mut state).shutdown();
    state.timer.wait_for_call();

    let exp_fin = Segment::new_empty(Fin, state.send_next, state.receive_next);
    expect_segment(&state, &exp_fin);

    // tc sends FIN
    state.timer.expect_start();
    let fin_from_tc =
        Segment::new_empty(Fin, state.receive_next, state.send_next);
    send_segment(&state, &fin_from_tc);
    state.timer.wait_for_call();

    // The FIN is retransmitted, but ack_num has been increased to ack
    // the FIN from the tc.
    let new_exp_fin = exp_fin.set_ack_num(state.receive_next + 1);
    expect_segment(&mut state, &new_exp_fin);

    // The tc acks the FIN
    let ack_to_fin =
        Segment::new_empty(Ack, state.receive_next + 1, state.send_next + 1);
    send_segment(&mut state, &ack_to_fin);

    wait_shutdown_complete(state);
}

#[test]
fn mf_close() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, b"some data");
    uut_write_with_tc_ack(&mut state, b"some data");

    state.uut_stream.take().unwrap().close();
    test_end_check(&mut state);
}

#[test]
fn af_uut_retransmits_fin() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_write_with_tc_ack(&mut state, b"some data");

    // Send FIN from uut
    state.timer.expect_start();
    let exp_fin = uut_shutdown(&mut state);
    state.timer.wait_for_call();

    // tc pretends it didn't get the FIN by not sending an ACK. Instead,
    // the timeout expires
    state.timer.re_expect_trigger_wait();

    expect_segment(&state, &exp_fin);

    state.timer.expect_stop();
    let ack = Segment::new_empty(Ack, state.receive_next, state.send_next);
    send_segment(&state, &ack);
    state.timer.wait_for_call();

    tc_shutdown(&mut state);
    wait_shutdown_complete(state);
}

#[test]
fn af_uut_retransmits_data_and_fin() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_write_with_tc_ack(&mut state, b"some initial data");

    let initial_send_next = state.send_next;

    // Send data from uut
    state.timer.expect_start();
    let (len, recv_data_seg) = uut_write(&mut state, b"some data");
    state.timer.wait_for_call();

    // Send FIN from uut
    let exp_fin = uut_shutdown(&mut state);

    // tc pretends it didn't get the data or FIN, so the timer expires
    state.timer.re_expect_trigger_wait();

    expect_segment(&state, &recv_data_seg);
    expect_segment(&state, &exp_fin);

    let data_seg_ack =
        Segment::new_empty(Ack, state.receive_next, initial_send_next + len);
    let fin_ack = Segment::new_empty(
        Ack,
        state.receive_next,
        initial_send_next + len + 1,
    );

    // Timer restarted because FIN is still not acked
    state.timer.expect_start();
    send_segment(&state, &data_seg_ack);
    state.timer.wait_for_call();

    state.timer.expect_stop();
    send_segment(&state, &fin_ack);
    state.timer.wait_for_call();

    tc_shutdown(&mut state);
    wait_shutdown_complete(state);
}

#[test]
fn af_tc_retransmits_fin() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, b"some data");
    uut_write_with_tc_ack(&mut state, b"some data to write");

    // Send FIN
    let (sent_fin, received_ack) = tc_shutdown(&mut state);

    // Re-transmit the FIN and get the same ack
    send_segment(&mut state, &sent_fin);
    expect_segment(&state, &received_ack);

    uut_write_with_tc_ack(&mut state, b"some other data to write");

    uut_shutdown_with_tc_ack__tc_already_shutdown(&mut state);
    wait_shutdown_complete(state);
}

#[test]
fn af_tc_retransmits_data_and_fin() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, b"some data");
    uut_write_with_tc_ack(&mut state, b"some data to write");

    // Send some data and a FIN
    let (sent_data_seg, _received_data_ack) =
        uut_read(&mut state, b"some data to read");
    let (sent_fin, received_fin_ack) = tc_shutdown(&mut state);

    // Re-transmit the data and FIN
    send_segment(&mut state, &sent_data_seg);
    expect_segment(&state, &received_fin_ack);
    send_segment(&mut state, &sent_fin);
    expect_segment(&state, &received_fin_ack);

    uut_write_with_tc_ack(&mut state, b"some other data to write");

    uut_shutdown_with_tc_ack__tc_already_shutdown(&mut state);
    wait_shutdown_complete(state);
}


////////////////////////////////////////////////////////////////////////////////
// Read/write test cases
////////////////////////////////////////////////////////////////////////////////

#[test]
fn mf_client_read_once() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, b"some data");

    shutdown(state);
}

#[test]
fn mf_client_read_multiple_times() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, b"first rweouinwrte");
    uut_read(&mut state, b"second hfuiasud");
    uut_read(&mut state, b"third uifdshufihsiughsyudfghkusfdf");
    uut_read(&mut state, b"fourth fuidshfadgaerge");
    uut_read(&mut state, b"fifth dhuifghuifdlfoiwejiow");
    uut_read(&mut state, b"sixth fdauykfudsfgs");
    uut_read(&mut state, b"seventh fsdhsdgfsd");
    uut_read(&mut state, b"eighth ijogifdgire");
    uut_read(&mut state, b"ninth ertwrw");
    uut_read(&mut state, b"tenth uhfsdghsu");

    shutdown(state);
}

#[test]
fn mf_client_write_once() {
    let mut state = setup_connected_uut_client();

    uut_write_with_tc_ack(&mut state, b"some data");

    shutdown(state);
}

#[test]
fn mf_client_write_multiple_times() {
    let mut state = setup_connected_uut_client();

    uut_write_with_tc_ack(&mut state, b"first agfs");
    uut_write_with_tc_ack(&mut state, b"second gfdhdgfh");
    uut_write_with_tc_ack(&mut state, b"third dfafsdfads");
    uut_write_with_tc_ack(&mut state, b"fourth dfafas");
    uut_write_with_tc_ack(&mut state, b"fifth dfasfasfsdaf");
    uut_write_with_tc_ack(&mut state, b"sixth thythrt");
    uut_write_with_tc_ack(&mut state, b"seventh fdsaref");
    uut_write_with_tc_ack(&mut state, b"eighth dagfsdrgrege");
    uut_write_with_tc_ack(&mut state, b"ninth asfaerger");
    uut_write_with_tc_ack(&mut state, b"tenth trehjk");

    shutdown(state);
}

#[test]
fn mf_client_reads_and_writes() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, b"first");
    uut_write_with_tc_ack(&mut state, b"second");
    uut_write_with_tc_ack(&mut state, b"third");
    uut_write_with_tc_ack(&mut state, b"fourth");
    uut_read(&mut state, b"fifth");
    uut_read(&mut state, b"sixth");
    uut_write_with_tc_ack(&mut state, b"seventh");

    shutdown(state);
}

#[test]
fn mf_simultaneous_read_and_write() {
    let mut state = setup_connected_uut_client();

    uut_write_with_tc_ack(&mut state, b"some data to write");
    uut_read(&mut state, b"some data to read");

    // uut sends some data
    let data_from_uut = b"data";
    let data_from_uut_len = data_from_uut.len() as u32;
    state.timer.expect_start();
    uut_stream(&mut state).write(data_from_uut).unwrap();
    state.timer.wait_for_call();

    let exp_data_seg =
        Segment::new(Ack, state.send_next, state.receive_next, data_from_uut);
    expect_segment(&state, &exp_data_seg);

    // tc sends some data "at the same time", i.e. it sends data before it
    // knows of the data the uut sent. So it doesn't ack that data.
    let data_from_tc = b"data from tc";
    let data_from_tc_len = data_from_tc.len() as u32;
    let seg_from_tc =
        Segment::new(Ack, state.receive_next, state.send_next, data_from_tc);

    // uut restarts the timer, because ack_num from tc is old, so nothing
    // was acked, and "fast retransmit" kicks in
    state.timer.expect_start();
    send_segment(&state, &seg_from_tc);
    state.timer.wait_for_call();

    expect_read(&mut state, &[data_from_tc]);

    // The uut retransmits its own data sent, but ack_num has been increased
    // to ack what it got from the tc.
    let new_exp_data_seg = exp_data_seg.set_ack_num(state.receive_next + data_from_tc_len);
    expect_segment(&state, &new_exp_data_seg);

    // Ack the data from uut
    state.timer.expect_stop();
    let ack_to_data_from_uut = Segment::new_empty(
        Ack,
        state.receive_next + data_from_tc_len,
        state.send_next + data_from_uut_len,
    );
    send_segment(&mut state, &ack_to_data_from_uut);
    state.timer.wait_for_call();

    state.send_next += data_from_uut_len;
    state.receive_next += data_from_tc_len;
    shutdown(state);
}

// TODO: Idea: when all test cases need a state parameter anyway, try
// to chain them together, running one scenario after the other
#[test]
fn af_uut_retransmits_data_due_to_timeout() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_write_with_tc_ack(&mut state, b"some initial data");

    let initial_send_next = state.send_next;

    // Send data from uut
    state.timer.expect_start();
    let data = b"some data";
    let (len, recv_seg) = uut_write(&mut state, data);
    state.timer.wait_for_call();

    // tc pretends it didn't get data by not sending an ACK. Instead,
    // the timeout expires
    state.timer.re_expect_trigger_wait();
    expect_segment(&state, &recv_seg);

    state.timer.expect_stop();
    let ack =
        Segment::new_empty(Ack, state.receive_next, initial_send_next + len);
    send_segment(&state, &ack);
    state.timer.wait_for_call();

    shutdown(state);
}

#[test]
fn af_uut_retransmits_multiple_data_segments_due_to_timeout() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_write_with_tc_ack(&mut state, b"some initial data");

    let initial_send_next = state.send_next;

    // Send data from uut
    state.timer.expect_start();
    let data1 = b"some data";
    let (len1, recv_seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call();

    // Note that the timer was only started for the first write
    let data2 = b"some other data";
    let (len2, recv_seg2) = uut_write(&mut state, data2);

    let data3 = b"some more data";
    let (len3, recv_seg3) = uut_write(&mut state, data3);

    // tc pretends it didn't get data by not sending an ACK. Instead,
    // the timeout expires
    state.timer.re_expect_trigger_wait();

    expect_segment(&state, &recv_seg1);
    expect_segment(&state, &recv_seg2);
    expect_segment(&state, &recv_seg3);

    let ack1 =
        Segment::new_empty(Ack, state.receive_next, initial_send_next + len1);
    let ack2 = Segment::new_empty(
        Ack,
        state.receive_next,
        initial_send_next + len1 + len2,
    );
    let ack3 = Segment::new_empty(
        Ack,
        state.receive_next,
        initial_send_next + len1 + len2 + len3,
    );

    state.timer.expect_start();
    send_segment(&state, &ack1);
    state.timer.wait_for_call();

    state.timer.expect_start();
    send_segment(&state, &ack2);
    state.timer.wait_for_call();

    state.timer.expect_stop();
    send_segment(&state, &ack3);
    state.timer.wait_for_call();

    shutdown(state);
}

#[test]
fn af_client_retransmits_data_due_to_old_ack() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_write_with_tc_ack(&mut state, b"some initial data");

    let initial_send_next = state.send_next;

    // Send data1 from uut
    state.timer.expect_start();
    let data1 = b"first data";
    let (len1, recv_seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call();

    // Send data2 from uut
    let data2 = b"second data";
    let (len2, recv_seg2) = uut_write(&mut state, data2);

    // tc pretends that it didn't get data1 by sending ACK (dup ack, fast
    // retransmit) for the original seq_num
    state.timer.expect_start();
    let send_ack0 =
        Segment::new_empty(Ack, state.receive_next, initial_send_next);
    send_segment(&state, &send_ack0);
    state.timer.wait_for_call();

    // This causes uut to retransmit everything from the acked seq_num to
    // "current"
    expect_segment(&state, &recv_seg1);
    expect_segment(&state, &recv_seg2);

    // Now the tc sends ack for both of them
    state.timer.expect_start();
    let send_ack1 =
        Segment::new_empty(Ack, state.receive_next, initial_send_next + len1);
    send_segment(&state, &send_ack1);
    state.timer.wait_for_call();

    state.timer.expect_stop();
    let send_ack2 = Segment::new_empty(
        Ack,
        state.receive_next,
        initial_send_next + len1 + len2,
    );
    send_segment(&state, &send_ack2);
    state.timer.wait_for_call();

    shutdown(state);
}

#[test]
fn af_first_segment_acked_but_not_second() {
    let mut state = setup_connected_uut_client();
    let initial_send_next = state.send_next;

    state.timer.expect_start();
    let data1 = b"some data";
    let (len1, _recv_seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call();

    let data2 = b"some other data";
    let (len2, recv_seg2) = uut_write(&mut state, data2);

    let ack1 =
        Segment::new_empty(Ack, state.receive_next, initial_send_next + len1);

    // TC sends Ack for the first segment, but not the second.
    // This causes the timer to be restarted because progress was made.
    state.timer.expect_start();
    send_segment(&state, &ack1);
    state.timer.wait_for_call();

    // But when the timer expires...
    state.timer.re_expect_trigger_wait();

    // ...only the unacked segment is retransmitted
    expect_segment(&state, &recv_seg2);

    state.timer.expect_stop();
    let ack2 = Segment::new_empty(
        Ack,
        state.receive_next,
        initial_send_next + len1 + len2,
    );
    send_segment(&state, &ack2);
    state.timer.wait_for_call();

    shutdown(state);
}

#[test]
fn af_cumulative_ack() {
    let mut state = setup_connected_uut_client();
    let initial_send_next = state.send_next;

    state.timer.expect_start();
    let data1 = b"some data";
    let (len1, _seg1) = uut_write(&mut state, data1);
    state.timer.wait_for_call();

    let data2 = b"some other data";
    let (len2, _seg2) = uut_write(&mut state, data2);

    let ack = Segment::new_empty(
        Ack,
        state.receive_next,
        initial_send_next + len1 + len2,
    );

    state.timer.expect_stop();
    send_segment(&state, &ack);
    state.timer.wait_for_call();

    shutdown(state);
}

#[test]
fn af_multi_segment_write() {
    let mut state = setup_connected_uut_client();

    // Generate some long data to write
    let len = MAXIMUM_SEGMENT_SIZE + 10 + rand::random::<u32>() % 100;
    assert!(len < MAXIMUM_SEGMENT_SIZE * 2);
    let data = random_data_of_length(len);

    // Send from uut
    state.timer.expect_start();
    let written_len = uut_stream(&mut state).write(&data).unwrap() as u32;
    assert_eq!(len, written_len);
    state.timer.wait_for_call();

    // Receive the first segment
    let exp_seg1 = Segment::new(
        Ack,
        state.send_next,
        state.receive_next,
        &data[0..MAXIMUM_SEGMENT_SIZE as usize],
    );
    expect_segment(&state, &exp_seg1);

    // Receive the second segment
    let exp_seg2 = Segment::new(
        Ack,
        state.send_next + MAXIMUM_SEGMENT_SIZE,
        state.receive_next,
        &data[MAXIMUM_SEGMENT_SIZE as usize..],
    );
    expect_segment(&state, &exp_seg2);

    state.send_next += len;

    state.timer.expect_stop();
    let ack = Segment::new_empty(Ack, state.receive_next, state.send_next);
    send_segment(&state, &ack);
    state.timer.wait_for_call();

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
fn af_same_segment_carries_data_and_acks() {
    let mut state = setup_connected_uut_client();

    // Send some data from uut
    state.timer.expect_start();
    let data_from_uut = b"some data";
    uut_write(&mut state, data_from_uut);
    state.timer.wait_for_call();

    // Then send some data from tc, without first sending a separate ACK
    state.timer.expect_stop();
    uut_read(&mut state, b"some other data");
    state.timer.wait_for_call();

    shutdown(state);
}

#[test]
fn af_out_of_order_segment() {
    let mut state = setup_connected_uut_client();

    let data1 = b"some data";
    let len1 = data1.len() as u32;
    let seg1 = Segment::new(Ack, state.receive_next, state.send_next, data1);

    let data2 = b"some other data";
    let len2 = data2.len() as u32;
    let seg2 =
        Segment::new(Ack, state.receive_next + len1, state.send_next, data2);

    send_segment(&state, &seg2);

    // ACK for the data before seg1 received
    let exp_ack = Segment::new_empty(Ack, state.send_next, state.receive_next);
    expect_segment(&state, &exp_ack);

    expect_read_no_data(&mut state);

    send_segment(&state, &seg1);

    // But when the gap is filled, the ACK acks everything sent
    let exp_ack_all = Segment::new_empty(
        Ack,
        state.send_next,
        state.receive_next + len1 + len2,
    );
    expect_segment(&state, &exp_ack_all);

    state.receive_next += len1 + len2;

    expect_read(&mut state, &[data1, data2]);

    shutdown(state);
}

#[test]
fn af_multiple_out_of_order_segments() {
    let mut state = setup_connected_uut_client();

    let mut segments = Vec::new();
    let len = 10;

    for segment_index in 0..9 {
        let data = random_data_of_length(len);
        let accumlative_len = segment_index * len;
        let seg = Segment::new(
            Ack,
            state.receive_next + accumlative_len,
            state.send_next,
            &data,
        );
        segments.push(seg);
    }

    let ack_base = Segment::new_empty(Ack, state.send_next, state.receive_next);

    // |  2      |
    send_segment(&state, &segments[2]);
    expect_segment(&state, &ack_base);
    expect_read_no_data(&mut state);

    // |  23     |
    send_segment(&state, &segments[3]);
    expect_segment(&state, &ack_base);
    expect_read_no_data(&mut state);

    // |  23  6  |
    send_segment(&state, &segments[6]);
    expect_segment(&state, &ack_base);
    expect_read_no_data(&mut state);

    // |  23  6 8|
    send_segment(&state, &segments[8]);
    expect_segment(&state, &ack_base);
    expect_read_no_data(&mut state);

    // |0 23  6 8|
    send_segment(&state, &segments[0]);
    let ack0 =
        Segment::new_empty(Ack, state.send_next, state.receive_next + len * 1);
    expect_segment(&state, &ack0);
    expect_read_data_of_segments(&mut state, &segments[0..=0]);

    // |0123  6 8|
    send_segment(&state, &segments[1]);
    let ack3 =
        Segment::new_empty(Ack, state.send_next, state.receive_next + len * 4);
    expect_segment(&state, &ack3);
    expect_read_data_of_segments(&mut state, &segments[1..=3]);

    // |0123 56 8|
    send_segment(&state, &segments[5]);
    expect_segment(&state, &ack3);
    expect_read_no_data(&mut state);

    // |0123456 8|
    send_segment(&state, &segments[4]);
    let ack6 =
        Segment::new_empty(Ack, state.send_next, state.receive_next + len * 7);
    expect_segment(&state, &ack6);
    expect_read_data_of_segments(&mut state, &segments[4..=6]);

    // |012345678|
    send_segment(&state, &segments[7]);
    let ack8 =
        Segment::new_empty(Ack, state.send_next, state.receive_next + len * 9);
    expect_segment(&state, &ack8);
    expect_read_data_of_segments(&mut state, &segments[7..=8]);

    state.receive_next += len * 9;

    shutdown(state);
}

#[test]
fn af_out_of_order_receive_buffer_full() {
    let mut state = setup_connected_uut_client();

    let mut segments = Vec::new();
    let num_segments = (MAXIMUM_RECV_BUFFER_SIZE + 1) as u32;
    let len = 20;

    for segment_index in 0..num_segments as u32 {
        let data = random_data_of_length(len);
        let accumlative_len = segment_index * len;
        let seg = Segment::new(
            Ack,
            state.receive_next + accumlative_len,
            state.send_next,
            &data,
        );
        segments.push(seg);
    }

    let ack_base = Segment::new_empty(Ack, state.send_next, state.receive_next);

    // Send all segments except the first
    for segment_index in 1..num_segments {
        send_segment(&state, &segments[segment_index as usize]);
        expect_segment(&state, &ack_base);
    }

    // Now send the first, and get an ack for all segments except the last
    send_segment(&state, &segments[0]);
    let offset_second_to_last = (num_segments - 1) * len;
    let ack_all_except_last = Segment::new_empty(
        Ack,
        state.send_next,
        state.receive_next + offset_second_to_last,
    );
    expect_segment(&state, &ack_all_except_last);

    // Now all data except the last can be read
    expect_read_data_of_segments(
        &mut state,
        &segments[0..(num_segments - 1) as usize],
    );

    state.receive_next += offset_second_to_last;

    shutdown(state);
}

#[test]
fn af_too_small_read_buffer() {
    let mut state = setup_connected_uut_client();

    uut_read(&mut state, b"some intial data");

    // Send some data
    let data = b"Some_data";
    send_from_tc(&mut state, data);

    // Read into a small buffer, not everything fits
    expect_read_with_buffer_len(&mut state, b"Some_", 5);

    // Read the rest
    expect_read_with_buffer_len(&mut state, b"data", 10);

    uut_read(&mut state, b"more data in the end");

    shutdown(state);
}

#[test]
fn af_tc_retransmits_data() {
    let mut state = setup_connected_uut_client();

    // Send some data
    let (sent_seg, received_ack) =
        uut_read(&mut state, b"some data to read");

    // Re-transmit the segment and get the same ack
    send_segment(&mut state, &sent_seg);
    expect_segment(&state, &received_ack);

    uut_read(&mut state, b"some other data");
    uut_write_with_tc_ack(&mut state, b"some data to write");

    shutdown(state);
}

#[test]
fn af_tc_retransmits_multiple_data_segments() {
    let mut state = setup_connected_uut_client();

    // Send two data segments
    let (sent_seg1, _received_ack1) =
        uut_read(&mut state, b"some data to read");
    let (sent_seg2, received_ack2) =
        uut_read(&mut state, b"other data being read");

    // Note that it's always the latest ack being sent
    send_segment(&mut state, &sent_seg1);
    expect_segment(&state, &received_ack2);
    send_segment(&mut state, &sent_seg2);
    expect_segment(&state, &received_ack2);

    uut_read(&mut state, b"some other data");
    uut_write_with_tc_ack(&mut state, b"some data to write");

    shutdown(state);
}

#[test]
fn af_ack_does_not_cause_ack_to_be_sent() {
    let mut state = setup_connected_uut_client();

    let (sent_ack1, _received_data_seg1) =
        uut_write_with_tc_ack(&mut state, b"datadatadata");
    let (sent_ack2, _received_data_seg2) =
        uut_write_with_tc_ack(&mut state, b"some other data to write");

    send_segment(&state, &sent_ack1);
    recv_check_no_data(&state.tc_socket);

    send_segment(&state, &sent_ack2);
    recv_check_no_data(&state.tc_socket);

    uut_write_with_tc_ack(&mut state, b"other");
    uut_read(&mut state, b"some data");

    shutdown(state);
}

////////////////////////////////////////////////////////////////////////////////
// Helper functions
////////////////////////////////////////////////////////////////////////////////

struct State {
    tc_socket: UdpSocket,
    uut_stream: Option<Stream>,
    uut_addr: SocketAddr,
    receive_next: u32,
    send_next: u32,
    timer: Arc<MockTimer>,
}

fn test_end_check(state: &mut State) {
    // To catch late calls to the timer
    std::thread::sleep(Duration::from_millis(1));
    assert!(state.uut_stream.is_none());
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
    timer.expect_stop();
    let join_handle = thread::Builder::new()
        .name("connect client".to_string())
        .spawn(move || {
            let init_seq_num = rand::random();
            Stream::connect_custom(timer_cloned, tc_addr, init_seq_num).unwrap()
        })
        .unwrap();

    // Receive SYN
    let (syn, uut_addr) = recv_segment_with_addr(&tc_socket);
    assert_eq!(Syn, syn.kind());

    // Send SYN-ACK
    let mut receive_next = rand::random();
    let send_next = syn.seq_num() + 1;
    let syn_ack = Segment::new_empty(SynAck, receive_next, send_next);
    send_segment_to(&tc_socket, uut_addr, &syn_ack);

    // Receive ACK
    let ack = recv_segment_from(&tc_socket, uut_addr);
    assert_eq!(Ack, ack.kind());
    receive_next += 1;
    assert_eq!(receive_next, ack.ack_num());

    let mut uut_stream = join_handle.join().unwrap();
    timer.wait_for_call();
    uut_stream.set_read_timeout(Some(Duration::from_millis(2)));

    State {
        tc_socket,
        uut_stream: Some(uut_stream),
        uut_addr,
        receive_next,
        send_next,
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
    // Check that the test case has read all data
    expect_read_no_data(&mut state);

    // And received all segments
    recv_check_no_data(&state.tc_socket);

    // Read and write some data to check that the uut is still working
    uut_read(&mut state, b"final data to read");
    uut_write_with_tc_ack(&mut state, b"final data to write");

    // Then do the shutdown
    uut_shutdown_with_tc_ack__tc_still_connected(&mut state);
    tc_shutdown(&mut state);
    wait_shutdown_complete(state);
}

fn uut_shutdown_with_tc_ack__tc_still_connected(state: &mut State) {
    uut_shutdown_with_tc_ack(state, true)
}

fn uut_shutdown_with_tc_ack__tc_already_shutdown(state: &mut State) {
    uut_shutdown_with_tc_ack(state, false)
}

fn uut_shutdown_with_tc_ack(state: &mut State, tc_still_connected: bool) {
    state.timer.expect_start();
    uut_shutdown(state);
    state.timer.wait_for_call();

    // If tc still connected, then uut sleeps forever once its FIN has been
    // acked. If tc not connected, then the uut thread stops, thus no sleep.
    if tc_still_connected {
        state.timer.expect_stop();
    }

    // tc sends ACK to the FIN
    let ack_to_fin =
        Segment::new_empty(Ack, state.receive_next, state.send_next);
    send_segment(&state, &ack_to_fin);

    if tc_still_connected {
        state.timer.wait_for_call();
    }
}

// This helper is supposed to mirror uut_write
fn uut_shutdown(state: &mut State) -> Segment {
    // Shutdown from the uut
    uut_stream(state).shutdown();

    // Recv FIN from the uut
    let exp_fin = Segment::new_empty(Fin, state.send_next, state.receive_next);
    expect_segment(&state, &exp_fin);
    state.send_next += 1;

    exp_fin
}

fn tc_shutdown(state: &mut State) -> (Segment, Segment) {
    // tc sends FIN
    let send_seg = Segment::new_empty(Fin, state.receive_next, state.send_next);
    send_segment(&state, &send_seg);
    state.receive_next += 1;

    // uut sends ACK to the FIN
    let exp_ack_to_fin =
        Segment::new_empty(Ack, state.send_next, state.receive_next);
    expect_segment(&state, &exp_ack_to_fin);

    // Since FIN has been received, 0 data is returned
    expect_read_with_buffer_len(state, b"", 123);
    // TODO: The code below triggers RecvError. Use it for a future test.
    // let mut buf = [0; 123];
    // let buf_before = buf.clone();
    // let read_len = uut_stream(state).read(&mut buf).unwrap();
    // assert_eq!(0, read_len);
    // assert_eq!(buf_before, buf);

    (send_seg, exp_ack_to_fin)
}

fn wait_shutdown_complete(mut state: State) {
    state.uut_stream.take().unwrap().wait_shutdown_complete();
    test_end_check(&mut state);
}

fn uut_read(state: &mut State, data: &[u8]) -> (Segment, Segment) {
    let segments = send_from_tc(state, data);

    // Check that the uut received the correct data
    expect_read(state, &[data]);

    segments
}

fn send_from_tc(
    state: &mut State,
    data: &[u8],
) -> (Segment, Segment) {
    // Send from the tc
    let send_seg = Segment::new(Ack, state.receive_next, state.send_next, data);
    send_segment(&state, &send_seg);
    state.receive_next += data.len() as u32;

    // Recv ACK from the uut
    let exp_ack = Segment::new_empty(Ack, state.send_next, state.receive_next);
    expect_segment(&state, &exp_ack);

    (send_seg, exp_ack)
}

fn expect_read_data_of_segments<'a, I>(state: &mut State, segments: I)
where
    I: IntoIterator<Item = &'a Segment>,
{
    let exp_datas: Vec<_> =
        segments.into_iter().map(|seg| seg.data()).collect();
    expect_read(state, &exp_datas);
}

fn expect_read(state: &mut State, exp_datas: &[&[u8]]) {
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

fn expect_read_with_buffer_len(
    state: &mut State,
    exp_data: &[u8],
    buffer_len: usize,
) {
    let mut buf = vec![0; buffer_len];
    let read_len = uut_stream(state).read(&mut buf).unwrap();
    assert_eq!(exp_data.len(), read_len);
    assert_eq!(&buf[0..exp_data.len()], exp_data);
}

fn uut_stream(state: &mut State) -> &mut Stream {
    state.uut_stream.as_mut().unwrap()
}

fn expect_read_no_data(state: &mut State) {
    let mut buf = [0; 1];
    let res = uut_stream(state).read(&mut buf);
    assert_eq!(ErrorKind::WouldBlock, res.unwrap_err().kind());
}

fn uut_write_with_tc_ack(state: &mut State, data: &[u8]) -> (Segment, Segment) {
    // Send from the uut
    state.timer.expect_start();
    let (_len, recv_data_seg) = uut_write(state, data);
    state.timer.wait_for_call();

    // Send ack from the tc
    state.timer.expect_stop();
    let send_ack = Segment::new_empty(Ack, state.receive_next, state.send_next);
    send_segment(&state, &send_ack);
    state.timer.wait_for_call();

    recv_check_no_data(&state.tc_socket);

    (send_ack, recv_data_seg)
}

fn uut_write(state: &mut State, data: &[u8]) -> (u32, Segment) {
    // Send from the uut
    let written_len = uut_stream(state).write(&data).unwrap();
    let len = data.len();
    assert_eq!(len, written_len);

    // Recv from the tc
    let exp_seg = Segment::new(Ack, state.send_next, state.receive_next, &data);
    expect_segment(&state, &exp_seg);
    state.send_next += len as u32;

    (len as u32, exp_seg)
}

fn expect_segment(state: &State, exp_seg: &Segment) {
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
