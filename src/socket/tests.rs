use super::*;

use async_std::future;
use std::io::{Error, ErrorKind};
use std::net::{UdpSocket, *};
use std::time::Duration;

use crate::controllable_timer::{self, Returner, Sleeper, Waiter};
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
// - Cumulative ack, one full segment, one byte more and one byte less
//   than the border
// - Send two segments, ack the first, only second is retransmitted

struct State {
    tc_socket: UdpSocket,
    uut_stream: Stream,
    uut_addr: SocketAddr,
    tc_seq_num: u32,
    uut_seq_num: u32,
    timer: Arc<MockTimer>,
}

impl Drop for State {
    fn drop(&mut self) {
        self.timer.test_end_check();
        recv_check_no_data(&mut self.tc_socket);
    }
}

struct MockTimer {
    sleep_expected: Mutex<bool>,
    sleep_called_tx: Sender<()>,
    sleep_called_rx: Receiver<()>,
    let_sleep_return_tx: Sender<()>,
    let_sleep_return_rx: Receiver<()>,
}

impl MockTimer {
    pub fn new() -> Self {
        let sleep_expected = Mutex::new(false);
        let (sleep_called_tx, sleep_called_rx) = async_channel::bounded(1);
        let (let_sleep_return_tx, let_sleep_return_rx) =
            async_channel::bounded(1);

        MockTimer {
            sleep_expected,
            sleep_called_tx,
            sleep_called_rx,
            let_sleep_return_tx,
            let_sleep_return_rx,
        }
    }

    pub fn expect_call_to_sleep(&self) {
        block_on(async {
            let mut locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(!*locked_sleep_expected);
            *locked_sleep_expected = true;
        });
    }

    pub fn wait_for_call_to_sleep(&self) {
        block_on(async {
            let duration = Duration::from_millis(10);
            let recv_sleep_called = self.sleep_called_rx.recv();
            let timeout_result =
                future::timeout(duration, recv_sleep_called).await;

            let recv_result =
                timeout_result.expect("Timeout waiting for sleep to be called");
            recv_result.expect("Error receiving from sleep_called_rx");
        });
    }

    pub fn trigger_and_expect_new_call(&self) {
        block_on(async {
            let mut locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(!*locked_sleep_expected);
            *locked_sleep_expected = true;

            self.let_sleep_return_tx.try_send(()).unwrap();
        });
    }

    // TODO: Move to drop when the Socket process is closed/FIN-ed
    pub fn test_end_check(&self) {
        block_on(async {
            let locked_sleep_expected = self.sleep_expected.lock().await;
            assert!(!*locked_sleep_expected);

            assert!(self.sleep_called_rx.is_empty());
        });
    }
}

#[async_trait]
impl Timer for MockTimer {
    async fn sleep(&self, duration: Duration) {
        assert_eq!(Duration::from_millis(100), duration);

        let mut locked_sleep_expected = self.sleep_expected.lock().await;
        assert!(*locked_sleep_expected);
        *locked_sleep_expected = false;
        drop(locked_sleep_expected);

        self.sleep_called_tx.try_send(()).unwrap();
        self.let_sleep_return_rx.recv().await.unwrap();
    }
}

#[test]
fn test_connect() {
    setup_connected_uut_client();
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
    let tc_addr = tc_socket.local_addr().unwrap();
    let uut_stream =
        Stream::connect_custom_timer(Arc::clone(&timer), tc_addr).unwrap();

    // Receive SYN
    let (syn, uut_addr) = recv_segment_from(&tc_socket);
    assert_eq!(Syn, syn.kind());

    // Send SYN-ACK
    let mut tc_seq_num = rand::random();
    let uut_seq_num = syn.seq_num() + 1;
    let syn_ack = Segment::new_empty(SynAck, tc_seq_num, uut_seq_num);
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
        timer,
    }
}

#[test]
fn test_client_read_once() {
    let mut state = setup_connected_uut_client();

    uut_complete_read(&mut state, "some data");
}

fn uut_complete_read(state: &mut State, string: &str) {
    let data = string.as_bytes();

    // Send from the tc
    let send_seg = Segment::new(Ack, state.tc_seq_num, state.uut_seq_num, data);
    send_segment(&state.tc_socket, state.uut_addr, &send_seg);
    state.tc_seq_num += data.len() as u32;

    // Recv ACK from the uut
    let recv_seg = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_ack = Segment::new_empty(Ack, state.uut_seq_num, state.tc_seq_num);
    assert_eq!(exp_ack, recv_seg);

    // Check that the uut received the correct data
    let read_data = uut_read_stream_once(&mut state.uut_stream);
    assert_eq!(data, read_data);
}

#[test]
fn test_client_read_multiple_times() {
    let mut state = setup_connected_uut_client();

    uut_complete_read(&mut state, "first rweouinwrte");
    uut_complete_read(&mut state, "second hfuiasud");
    uut_complete_read(&mut state, "third uifdshufihsiughsyudfghkusfdf");
    uut_complete_read(&mut state, "fourth fuidshfadgaerge");
    uut_complete_read(&mut state, "fifth dhuifghuifdlfoiwejiow");
    uut_complete_read(&mut state, "sixth fdauykfudsfgs");
    uut_complete_read(&mut state, "seventh fsdhsdgfsd");
    uut_complete_read(&mut state, "eighth ijogifdgire");
    uut_complete_read(&mut state, "ninth ertwrw");
    uut_complete_read(&mut state, "tenth uhfsdghsu");
}

#[test]
fn test_client_write_once() {
    let mut state = setup_connected_uut_client();

    uut_complete_write(&mut state, b"some data");
}

fn uut_complete_write(state: &mut State, data: &[u8]) {
    // Send from the uut
    state.timer.expect_call_to_sleep();
    let len = uut_write(state, data);
    state.timer.wait_for_call_to_sleep();

    // Recv from the tc
    let recv_seg = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg = Segment::new(Ack, state.uut_seq_num, state.tc_seq_num, &data);
    assert_eq!(exp_seg, recv_seg);
    state.uut_seq_num += len as u32;

    // Send ack from the tc
    let send_seg = Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num);
    send_segment(&state.tc_socket, state.uut_addr, &send_seg);

    recv_check_no_data(&state.tc_socket);
}

#[test]
fn test_client_write_multiple_times() {
    let mut state = setup_connected_uut_client();

    uut_complete_write(&mut state, b"first agfs");
    uut_complete_write(&mut state, b"second gfdhdgfh");
    uut_complete_write(&mut state, b"third dfafsdfads");
    uut_complete_write(&mut state, b"fourth dfafas");
    uut_complete_write(&mut state, b"fifth dfasfasfsdaf");
    uut_complete_write(&mut state, b"sixth thythrt");
    uut_complete_write(&mut state, b"seventh fdsaref");
    uut_complete_write(&mut state, b"eighth dagfsdrgrege");
    uut_complete_write(&mut state, b"ninth asfaerger");
    uut_complete_write(&mut state, b"tenth trehjk");
}

#[test]
fn test_client_reads_and_writes() {
    let mut state = setup_connected_uut_client();

    uut_complete_read(&mut state, "first");
    uut_complete_write(&mut state, b"second");
    uut_complete_write(&mut state, b"third");
    uut_complete_write(&mut state, b"fourth");
    uut_complete_read(&mut state, "fifth");
    uut_complete_read(&mut state, "sixth");
    uut_complete_write(&mut state, b"seventh");
}

#[test]
fn test_client_write_retransmit_due_to_timeout() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_complete_write(&mut state, b"some initial data");

    // Send data from uut
    state.timer.expect_call_to_sleep();
    let data = b"some data";
    let len = uut_write(&mut state, data);
    state.timer.wait_for_call_to_sleep();

    // Recv data from the tc
    let recv_seg1 = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg = Segment::new(Ack, state.uut_seq_num, state.tc_seq_num, data);
    assert_eq!(exp_seg, recv_seg1);

    // tc pretends it didn't get data by not sending an ACK. Instead,
    // the timeout expires
    state.timer.trigger_and_expect_new_call();

    let recv_seg2 = recv_segment(&state.tc_socket, state.uut_addr);
    assert_eq!(exp_seg, recv_seg2);
    state.timer.wait_for_call_to_sleep();

    let ack =
        Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num + len);
    send_segment(&state.tc_socket, state.uut_addr, &ack);
}

#[test]
fn test_client_write_retransmit_multiple_segments_due_to_timeout() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_complete_write(&mut state, b"some initial data");

    // Send data from uut
    state.timer.expect_call_to_sleep();
    let data1 = "some data".as_bytes();
    let len1 = uut_write(&mut state, data1);
    state.timer.wait_for_call_to_sleep();

    // Note that the timer was only started for the first write
    let data2 = "some other data".as_bytes();
    let len2 = uut_write(&mut state, data2);
    let data3 = "some more data".as_bytes();
    let len3 = uut_write(&mut state, data3);

    // Recv data from the tc
    let recv_seg1 = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg1 =
        Segment::new(Ack, state.uut_seq_num, state.tc_seq_num, &data1);
    assert_eq!(exp_seg1, recv_seg1);

    let recv_seg2 = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg2 =
        Segment::new(Ack, state.uut_seq_num + len1, state.tc_seq_num, &data2);
    assert_eq!(exp_seg2, recv_seg2);

    let recv_seg3 = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg3 = Segment::new(
        Ack,
        state.uut_seq_num + len1 + len2,
        state.tc_seq_num,
        &data3,
    );
    assert_eq!(exp_seg3, recv_seg3);

    // tc pretends it didn't get data by not sending an ACK. Instead,
    // the timeout expires
    state.timer.trigger_and_expect_new_call();
    state.timer.wait_for_call_to_sleep();

    let recv_seg1_retrans = recv_segment(&state.tc_socket, state.uut_addr);
    assert_eq!(exp_seg1, recv_seg1_retrans);
    let recv_seg2_retrans = recv_segment(&state.tc_socket, state.uut_addr);
    assert_eq!(exp_seg2, recv_seg2_retrans);
    let recv_seg3_retrans = recv_segment(&state.tc_socket, state.uut_addr);
    assert_eq!(exp_seg3, recv_seg3_retrans);

    let ack1 =
        Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num + len1);
    let ack2 = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        state.uut_seq_num + len1 + len2,
    );
    let ack3 = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        state.uut_seq_num + len1 + len2 + len3,
    );

    state.timer.expect_call_to_sleep();
    send_segment(&state.tc_socket, state.uut_addr, &ack1);
    state.timer.wait_for_call_to_sleep();

    state.timer.expect_call_to_sleep();
    send_segment(&state.tc_socket, state.uut_addr, &ack2);
    state.timer.wait_for_call_to_sleep();

    send_segment(&state.tc_socket, state.uut_addr, &ack3);
}

#[test]
fn test_client_write_retransmit_due_to_old_ack() {
    let mut state = setup_connected_uut_client();

    // Send some data successfully. This is to check that this data
    // isn't retransmitted
    uut_complete_write(&mut state, b"some initial data");

    // Send data1 from uut
    state.timer.expect_call_to_sleep();
    let data1 = b"first data";
    let len1 = uut_write(&mut state, data1);
    state.timer.wait_for_call_to_sleep();

    // Recv data1 from the tc
    let recv_seg1 = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg1 =
        Segment::new(Ack, state.uut_seq_num, state.tc_seq_num, data1);
    assert_eq!(exp_seg1, recv_seg1);

    // Send data2 from uut
    let data2 = b"second data";
    let len2 = uut_write(&mut state, data2);

    // Recv data2 from the tc
    let recv_seg2 = recv_segment(&state.tc_socket, state.uut_addr);
    let exp_seg2 = Segment::new(
        Ack,
        state.uut_seq_num + len1 as u32,
        state.tc_seq_num,
        data2,
    );
    assert_eq!(exp_seg2, recv_seg2);

    // tc pretends that it didn't get data1 by sending ACK (dup ack, fast
    // retransmit) for the original seq_num
    state.timer.expect_call_to_sleep();
    let send_ack0 =
        Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num);
    send_segment(&state.tc_socket, state.uut_addr, &send_ack0);

    // This causes uut to retransmit everything from the acked seq_num to
    // "current"
    state.timer.wait_for_call_to_sleep();
    let recv_seg1_retransmit = recv_segment(&state.tc_socket, state.uut_addr);
    assert_eq!(exp_seg1, recv_seg1_retransmit);
    let recv_seg2_retransmit = recv_segment(&state.tc_socket, state.uut_addr);
    assert_eq!(exp_seg2, recv_seg2_retransmit);

    // Now the tc sends ack for both of them
    let send_ack1 =
        Segment::new_empty(Ack, state.tc_seq_num, state.uut_seq_num + len1);
    send_segment(&state.tc_socket, state.uut_addr, &send_ack1);
    let send_ack2 = Segment::new_empty(
        Ack,
        state.tc_seq_num,
        state.uut_seq_num + len1 + len2,
    );
    send_segment(&state.tc_socket, state.uut_addr, &send_ack2);
}

// TODO: Only stream, not state, as argument
fn uut_write(state: &mut State, data: &[u8]) -> u32 {
    let written_len = state.uut_stream.write(&data).unwrap();
    let len = data.len();
    assert_eq!(len, written_len);
    len as u32
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

fn recv_check_no_data(tc_socket: &UdpSocket) {
    let mut buf = [0; 4096];
    match tc_socket.recv(&mut buf) {
        Err(err) => assert_eq!(ErrorKind::WouldBlock, err.kind()),
        other => {
            panic!("Didn't get WouldBlock, instead got: {:?}", other);
        }
    };
}

fn send_segment(
    tc_socket: &UdpSocket,
    uut_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    tc_socket.send_to(&encoded_seq, uut_addr).unwrap();
}

// Possibly "complete" should be renamed "main flow"
fn uut_read_stream_once(stream: &mut Stream) -> Vec<u8> {
    let mut buf = [0; 4096];
    let amt = stream.read(&mut buf).unwrap();
    buf[0..amt].to_vec()
}

// Possibile "white box" things, or "test window" things
// - Number of discarded received segments
// - Buffer size
