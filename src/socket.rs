#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use async_std::net::*;
use async_trait::async_trait;
use futures::executor::block_on;
use futures::lock::{Mutex, MutexGuard};
use futures::{future::FutureExt, pin_mut, select};
use std::io::Result;
use std::str;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::segment::Kind::*;
use crate::segment::Segment;

const RETRANSMISSION_TIMER: Duration = Duration::from_millis(100);
const MAXIMUM_SEGMENT_SIZE: u32 = 500;

#[async_trait]
trait Timer {
    async fn sleep(&self, duration: Duration);
}

struct PlainTimer {}

#[async_trait]
impl Timer for PlainTimer {
    async fn sleep(&self, duration: Duration) {
        async_std::task::sleep(duration).await;
    }
}

pub struct Listener {
    udp_socket: Arc<UdpSocket>,
}

pub struct Stream {
    inner_stream: InnerStream,
}

#[derive(Debug, PartialEq)]
pub enum CloseResult {
    AllDataSent,
    DataRemaining,
}

enum InnerStream {
    Server(ServerStream),
    Client(ClientStream),
}

struct ServerStream {
    udp_socket: Arc<UdpSocket>,
    peer_addr: SocketAddr,
}

struct ClientStream {
    read_rx: Receiver<Vec<u8>>,
    user_action_tx: Sender<UserAction>,
    join_handle: JoinHandle<()>,
    state: Arc<Mutex<ConnectedState>>,
}

enum UserAction {
    SendData(Vec<u8>),
    Shutdown,
    Close,
}

struct ConnectedState {
    send_next: u32,
    receive_next: u32,
    fin_sent: bool,
    fin_received: bool,
    buffer: Vec<Segment>,
}

type LockedConnectedState<'a> = MutexGuard<'a, ConnectedState>;

impl Listener {
    pub fn bind<A: ToSocketAddrs>(local_addr: A) -> Result<Listener> {
        match block_on(UdpSocket::bind(local_addr)) {
            Ok(udp_socket) => {
                println!("Listener started");
                let listener = Listener {
                    udp_socket: Arc::new(udp_socket),
                };
                Ok(listener)
            }
            Err(err) => Err(err),
        }
    }

    pub fn accept(&self) -> Result<(Stream, SocketAddr)> {
        println!("accept called");
        let mut buf = [0; 4096];
        let (amt, peer_addr) =
            block_on(self.udp_socket.recv_from(&mut buf)).unwrap();

        let string = str::from_utf8(&buf[0..amt]).unwrap();

        if string == "syn" {
            println!("Got syn from {:?}", peer_addr);
            let stream =
                Stream::accept(Arc::clone(&self.udp_socket), peer_addr)
                    .unwrap();
            Ok((stream, peer_addr))
        } else {
            println!("non-syn received {:?}", string);
            unimplemented!()
        }
    }
}

impl Stream {
    pub fn connect<A: ToSocketAddrs>(peer_addr: A) -> Result<Stream> {
        // TODO: See if Arc can be removed in non-test code
        let timer = Arc::new(PlainTimer {});
        Self::connect_custom_timer(timer, peer_addr)
    }

    fn connect_custom_timer<
        T: Timer + Send + Sync + 'static,
        A: ToSocketAddrs,
    >(
        timer: Arc<T>,
        peer_addr: A,
    ) -> Result<Stream> {
        let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();

        let (read_tx, read_rx) = async_channel::unbounded();
        let (user_action_tx, user_action_rx) = async_channel::unbounded();

        match block_on(UdpSocket::bind(local_addr)) {
            Ok(udp_socket) => {
                let (send_next, receive_next) = block_on(async {
                    UdpSocket::connect(&udp_socket, peer_addr).await.unwrap();
                    Self::client_handshake(&udp_socket).await
                });

                let state = ConnectedState {
                    send_next,
                    receive_next,
                    fin_sent: false,
                    fin_received: false,
                    buffer: Vec::new(),
                };

                let state_in_mutex = Mutex::new(state);
                let state_in_arc = Arc::new(state_in_mutex);
                let state_for_loop = Arc::clone(&state_in_arc);

                let join_handle = thread::Builder::new()
                    .name("client".to_string())
                    .spawn(move || {
                        block_on(connected_loop(
                            timer,
                            state_for_loop,
                            udp_socket,
                            read_tx,
                            user_action_rx,
                        ))
                    })
                    .unwrap();
                let client_stream = ClientStream {
                    read_rx,
                    user_action_tx,
                    join_handle,
                    state: state_in_arc,
                };
                let inner_stream = InnerStream::Client(client_stream);
                let stream = Stream { inner_stream };
                Ok(stream)
            }
            Err(err) => Err(err),
        }
    }

    async fn client_handshake(udp_socket: &UdpSocket) -> (u32, u32) {
        // Send SYN
        let send_next = rand::random();
        let syn = Segment::new_empty(Syn, send_next, 0);
        let encoded_syn = Segment::encode(&syn);

        udp_socket.send(&encoded_syn).await.unwrap();

        // Receive SYN-ACK
        let mut buf = [0; 4096];
        let amt = udp_socket.recv(&mut buf).await.unwrap();

        let syn_ack = Segment::decode(&buf[0..amt]).unwrap();

        let new_send_next = send_next + 1;
        // TODO: Handle error instead
        assert_eq!(new_send_next, syn_ack.ack_num());
        assert_eq!(SynAck, syn_ack.kind());
        assert_eq!(0, syn_ack.data().len());

        let receive_next = syn_ack.seq_num() + 1;

        // Send ACK
        let ack = Segment::new_empty(Ack, new_send_next, receive_next);
        let encoded_ack = Segment::encode(&ack);

        udp_socket.send(&encoded_ack).await.unwrap();

        (new_send_next, receive_next)
    }

    fn accept<A: ToSocketAddrs>(
        udp_socket: Arc<UdpSocket>,
        to_peer_addr: A,
    ) -> Result<Stream> {
        let peer_addr = block_on(to_peer_addr.to_socket_addrs())
            .unwrap()
            .last()
            .unwrap();

        block_on(udp_socket.send_to("ack".as_bytes(), peer_addr)).unwrap();

        let stream = ServerStream {
            udp_socket,
            peer_addr,
        };

        Ok(Self::pack_server_stream(stream))
    }

    fn pack_server_stream(server_stream: ServerStream) -> Stream {
        Stream {
            inner_stream: InnerStream::Server(server_stream),
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match &mut self.inner_stream {
            InnerStream::Client(client_stream) => {
                ClientStream::write(client_stream, buf)
            }
            InnerStream::Server(server_stream) => {
                ServerStream::write(server_stream, buf)
            }
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &mut self.inner_stream {
            InnerStream::Client(client_stream) => {
                ClientStream::read(client_stream, buf)
            }
            InnerStream::Server(server_stream) => {
                ServerStream::read(server_stream, buf)
            }
        }
    }

    pub fn shutdown(&mut self) {
        if let InnerStream::Client(client_stream) = &self.inner_stream {
            block_on(client_stream.user_action_tx.send(UserAction::Shutdown))
                .unwrap();
        }
    }

    pub fn wait_shutdown_complete(self) {
        if let InnerStream::Client(client_stream) = self.inner_stream {
            client_stream.join_handle.join().unwrap();
        }
    }

    pub fn close(self) {
        if let InnerStream::Client(client_stream) = self.inner_stream {
            block_on(client_stream.user_action_tx.send(UserAction::Close))
                .unwrap();
            client_stream.join_handle.join().unwrap();
        }
    }
}

impl ClientStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        assert!(buf.len() > 0);
        block_on(self.user_action_tx.send(UserAction::SendData(buf.to_vec())))
            .unwrap();
        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let data = block_on(self.read_rx.recv()).unwrap();
        let len = data.len();

        // TODO: Solve the horrible inefficiency
        for i in 0..len {
            buf[i] = data[i];
        }

        Ok(len)
    }
}

impl ServerStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        block_on(self.udp_socket.send_to(buf, self.peer_addr))
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        println!("server read from {:?}", self.udp_socket.local_addr());
        block_on(self.udp_socket.recv(buf))
    }
}

async fn connected_loop<T: Timer>(
    timer: Arc<T>,
    state: Arc<Mutex<ConnectedState>>,
    udp_socket: UdpSocket,
    read_tx: Sender<Vec<u8>>,
    user_action_rx: Receiver<UserAction>,
) {
    let recv_socket_state = RecvSocketState {
        udp_socket: &udp_socket,
        read_tx,
        connected_state: Arc::clone(&state),
    };
    let recv_user_action_rx_state = RecvWriteRxState {
        udp_socket: &udp_socket,
        user_action_rx,
        connected_state: Arc::clone(&state),
    };

    let future_recv_socket = recv_socket(recv_socket_state).fuse();
    let future_recv_user_action_rx =
        recv_user_action_rx(recv_user_action_rx_state).fuse();
    let future_timeout = timeout(&timer, true).fuse();
    // Need a separate timeout future. recv_socket returns a bool indicating
    // if the timeout future so be cleared or not. recv_user_action_rx returns
    // Option<Future> which replaces the old timeout future.

    // TODO: Maybe move to connected state. Maybe it can be cleaned up
    // in some other better way.
    let mut timer_running = false;
    pin_mut!(
        future_recv_socket,
        future_recv_user_action_rx,
        future_timeout
    );
    loop {
        select! {
            new_recv_socket_state = future_recv_socket => {
                if let RecvSocketResult::Continue(new_recv_socket_state, restart_timer) = new_recv_socket_state {
                    let new_future_recv_socket =
                        recv_socket(new_recv_socket_state).fuse();
                    future_recv_socket.set(new_future_recv_socket);

                    // TODO: enum with Restart and Cancel
                    if restart_timer {
                        future_timeout.set(timeout(&timer, false).fuse());
                        timer_running = true;
                    } else {
                        // TODO: Remove/cancel instead
                        future_timeout.set(timeout(&timer, true).fuse());
                        timer_running = false;
                    }
                } else {
                    return;
                }
            },

            result = future_recv_user_action_rx => {
                if let RecvWriteRxResult::Continue(new_user_action_rx_state, start_timer) = result {
                    let new_future_recv_user_action_rx =
                        recv_user_action_rx(new_user_action_rx_state).fuse();
                    future_recv_user_action_rx.set(new_future_recv_user_action_rx);

                    // let locked_connected_state = connected_state_in_arc.lock().await;
                    // let buffer = &locked_connected_state.buffer;
                    // assert_eq!(timer_running, buffer.len() > 0);
                    if !timer_running && start_timer {
                        future_timeout.set(timeout(&timer, false).fuse());
                        timer_running = true;
                    }
                } else {
                    return;
                }
            },

            _ = future_timeout => {
                let locked_connected_state = state.lock().await;

                for segment in &locked_connected_state.buffer {
                    send_segment(
                        &udp_socket,
                        udp_socket.peer_addr().unwrap(),
                        &segment
                    ).await;
                }

                future_timeout.set(timeout(&timer, false).fuse());
            }
        };
    }
}

async fn timeout<T: Timer>(timer: &Arc<T>, forever: bool) {
    if forever {
        async_std::task::sleep(Duration::from_secs(1000000000)).await;
    } else {
        timer.sleep(RETRANSMISSION_TIMER).await;
    }
}

enum RecvSocketResult<'a> {
    Continue(RecvSocketState<'a>, bool),
    Exit,
}

struct RecvSocketState<'a> {
    udp_socket: &'a UdpSocket,
    read_tx: Sender<Vec<u8>>,
    connected_state: Arc<Mutex<ConnectedState>>,
}

async fn recv_socket(state: RecvSocketState<'_>) -> RecvSocketResult {
    let peer_addr = state.udp_socket.peer_addr().unwrap();
    let segment = recv_segment(state.udp_socket, peer_addr).await;

    let mut locked_connected_state = state.connected_state.lock().await;

    // TODO: Reject invalid segments instead
    // The segment shouldn't ack something not sent
    assert!(locked_connected_state.send_next >= segment.ack_num());

    // The segment should contain the next expected data
    assert_eq!(locked_connected_state.receive_next, segment.seq_num());

    let ack_num = segment.ack_num();
    let seq_num = segment.seq_num();
    let data = segment.to_data();
    let len = data.len() as u32;

    // TODO: If segment.seq_num doesn't match state.receive_next, disacrd it
    assert_eq!(locked_connected_state.receive_next, seq_num);
    locked_connected_state.receive_next += len;

    let segments_remain = handle_retransmissions_at_ack_recv(
        ack_num,
        &mut locked_connected_state,
        &state,
    )
    .await;

    let ack = Segment::new_empty(
        Ack,
        locked_connected_state.send_next,
        locked_connected_state.receive_next,
    );

    if !segments_remain && locked_connected_state.fin_sent {
        RecvSocketResult::Exit
    } else if len == 0 {
        drop(locked_connected_state);
        RecvSocketResult::Continue(state, segments_remain)
    } else {
        send_segment(state.udp_socket, peer_addr, &ack).await;

        drop(locked_connected_state);
        match state.read_tx.send(data).await {
            Ok(()) => RecvSocketResult::Continue(state, segments_remain),
            Err(_) => RecvSocketResult::Exit,
        }
    }
}

async fn handle_retransmissions_at_ack_recv(
    ack_num: u32,
    locked_connected_state: &mut LockedConnectedState<'_>,
    state: &RecvSocketState<'_>,
) -> bool {
    let buffer = &mut locked_connected_state.buffer;

    let buffer_len_before = buffer.len();
    removed_acked_segments(ack_num, buffer);
    let buffer_len_after = buffer.len();

    let peer_addr = state.udp_socket.peer_addr().unwrap();

    // Make sure this if statement is thoroughly tested
    assert!(buffer_len_after <= buffer_len_before);
    let made_progress = buffer_len_after < buffer_len_before;
    let segments_remain = buffer_len_after > 0;
    let need_retransmit = segments_remain && !made_progress;
    if need_retransmit {
        for seg in buffer {
            send_segment(state.udp_socket, peer_addr, &seg).await;
        }
    }
    // If segments remain, restart the timer, because if something was acked,
    // we made progresss, and if something wasn't acked, we retransmit
    segments_remain
}

fn removed_acked_segments(ack_num: u32, buffer: &mut Vec<Segment>) {
    while buffer.len() > 0 {
        let first_unacked_segment = &buffer[0];
        if ack_num
            >= first_unacked_segment.seq_num()
                + first_unacked_segment.data().len() as u32
        {
            buffer.remove(0);
        } else {
            break;
        }
    }
}

enum RecvWriteRxResult<'a> {
    Continue(RecvWriteRxState<'a>, bool),
    Exit,
}

struct RecvWriteRxState<'a> {
    udp_socket: &'a UdpSocket,
    user_action_rx: Receiver<UserAction>,
    connected_state: Arc<Mutex<ConnectedState>>,
}

async fn recv_user_action_rx(state: RecvWriteRxState<'_>) -> RecvWriteRxResult {
    match state.user_action_rx.recv().await {
        Ok(user_action) => {
            let mut locked_connected_state = state.connected_state.lock().await;

            match user_action {
                UserAction::Shutdown => {
                    let seg = Segment::new_empty(
                        Fin,
                        locked_connected_state.send_next,
                        locked_connected_state.receive_next,
                    );

                    let peer_addr = state.udp_socket.peer_addr().unwrap();
                    send_segment(state.udp_socket, peer_addr, &seg).await;

                    locked_connected_state.buffer.push(seg);

                    locked_connected_state.fin_sent = true;
                }
                UserAction::SendData(data) => {
                    for chunk in data.chunks(MAXIMUM_SEGMENT_SIZE as usize) {
                        let seg = Segment::new(
                            Ack,
                            locked_connected_state.send_next,
                            locked_connected_state.receive_next,
                            &chunk,
                        );

                        let peer_addr = state.udp_socket.peer_addr().unwrap();
                        send_segment(state.udp_socket, peer_addr, &seg).await;

                        locked_connected_state.send_next += chunk.len() as u32;
                        locked_connected_state.buffer.push(seg);
                    }
                }
                UserAction::Close => {
                    // TODO: Avoid using early return
                    return RecvWriteRxResult::Exit;
                }
            }

            // TODO: The below should always be true?
            let start_timer = locked_connected_state.buffer.len() > 0;

            drop(locked_connected_state);
            RecvWriteRxResult::Continue(state, start_timer)
        }
        Err(_) => RecvWriteRxResult::Exit,
    }
}

async fn recv_segment(
    udp_socket: &UdpSocket,
    peer_addr: SocketAddr,
) -> Segment {
    let mut buf = [0; 4096];
    let (amt, recv_addr) = udp_socket.recv_from(&mut buf).await.unwrap();
    assert_eq!(peer_addr, recv_addr);
    Segment::decode(&buf[0..amt]).unwrap()
}

async fn send_segment(
    udp_socket: &UdpSocket,
    _peer_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    udp_socket.send(&encoded_seq).await.unwrap();
}
