#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use async_std::future;
use async_std::net::*;
use async_trait::async_trait;
use either::Either;
use futures::executor::block_on;
use futures::future::select_all;
use futures::lock::{Mutex, MutexGuard};
use futures::{future::FutureExt, pin_mut, select};
use std::collections::BTreeMap;
use std::io::{Error, ErrorKind, Read, Result};
use std::pin::Pin;
use std::str;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::segment::Kind::*;
use crate::segment::Segment;

//////////////////////////////////////////////////////////////////
// Constants
//////////////////////////////////////////////////////////////////

const RETRANSMISSION_TIMER: Duration = Duration::from_millis(100);
const MAXIMUM_SEGMENT_SIZE: u32 = 500;
const MAXIMUM_RECV_BUFFER_SIZE: usize = 10;

//////////////////////////////////////////////////////////////////
// Timer
//////////////////////////////////////////////////////////////////

#[async_trait]
trait Timer {
    async fn sleep(&self, duration: SleepDuration);
}

#[derive(Debug, PartialEq)]
enum SleepDuration {
    Finite(Duration),
    Forever,
}

struct PlainTimer {}

#[async_trait]
impl Timer for PlainTimer {
    async fn sleep(&self, duration: SleepDuration) {
        match duration {
            SleepDuration::Finite(duration) => {
                async_std::task::sleep(duration).await;
            }
            SleepDuration::Forever => {
                async_std::task::sleep(Duration::from_secs(1000000000)).await;
            }
        }
    }
}

//////////////////////////////////////////////////////////////////
// Public structs and interface
//////////////////////////////////////////////////////////////////

pub struct Listener {
    local_addr: SocketAddr,
    user_action_tx: Sender<UserAction>,
    join_handle: JoinHandle<()>,
    accept_rx: Receiver<(Stream, SocketAddr)>,
}

pub struct Stream {
    peer_action_rx: Receiver<PeerAction>,
    user_action_tx: Sender<UserAction>,
    shutdown_complete: Either<JoinHandle<()>, Receiver<()>>,
    shutdown_sent: bool,
    read_timeout: Option<Duration>,
    last_data: Option<Vec<u8>>,
    local_addr: SocketAddr,
}

#[derive(Debug, PartialEq)]
pub enum CloseResult {
    AllDataSent,
    DataRemaining,
}

enum PeerAction {
    Data(Vec<u8>),
    EOF,
}

#[derive(Debug)]
enum UserAction {
    SendData(Vec<u8>),
    Shutdown,
    Close,
    Accept,
}

struct CustomAcceptData<T> {
    timer: Arc<T>,
    init_seq_num: u32,
}

impl<T> Clone for CustomAcceptData<T> {
    fn clone(&self) -> CustomAcceptData<T> {
        CustomAcceptData {
            timer: Arc::clone(&self.timer),
            init_seq_num: self.init_seq_num,
        }
    }
}

impl Listener {
    pub fn bind<A: ToSocketAddrs>(local_addr: A) -> Result<Listener> {
        Self::bind_custom::<A, PlainTimer>(local_addr, None)
    }

    fn bind_custom<A: ToSocketAddrs, T: Timer + Send + Sync + 'static>(
        local_addr: A,
        custom_accept_data: Option<Vec<CustomAcceptData<T>>>,
    ) -> Result<Listener> {
        match block_on(UdpSocket::bind(local_addr)) {
            Ok(udp_socket) => {
                // println!("Listener started");
                let local_addr = udp_socket.local_addr().unwrap();
                let (user_action_tx, user_action_rx) =
                    async_channel::unbounded();
                let (accept_tx, accept_rx) = async_channel::unbounded();

                let udp_socket = Arc::new(udp_socket);

                let join_handle = thread::Builder::new()
                    .name("server".to_string())
                    .spawn(move || {
                        let mut server = Server::new(
                            Arc::clone(&udp_socket),
                            accept_tx,
                            custom_accept_data,
                        );

                        block_on(server.server_loop(
                            Arc::clone(&udp_socket),
                            &user_action_rx,
                        ))
                    })
                    .unwrap();

                let listener = Listener {
                    local_addr,
                    user_action_tx,
                    join_handle,
                    accept_rx,
                };
                Ok(listener)
            }
            Err(err) => Err(err),
        }
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.local_addr)
    }

    pub fn accept(&self) -> Result<(Stream, SocketAddr)> {
        let (stream, peer_addr) = block_on(self.accept_rx.recv()).unwrap();
        Ok((stream, peer_addr))
    }

    pub fn shutdown_all(&mut self) {
        block_on(self.user_action_tx.send(UserAction::Shutdown)).unwrap();
    }

    pub fn wait_shutdown_complete(self) {
        self.join_handle.join().unwrap();
    }
}

#[derive(Debug)]
enum ServerSelectResult {
    RecvSocket(Segment, SocketAddr),
    RecvUserAction(Option<UserAction>),
}

impl Stream {
    pub fn connect<A: ToSocketAddrs>(peer_addr: A) -> Result<Stream> {
        // TODO: See if Arc can be removed in non-test code
        let timer = Arc::new(PlainTimer {});
        let init_seq_num = rand::random();
        Self::connect_custom(timer, peer_addr, init_seq_num)
    }

    fn connect_custom<T: Timer + Send + Sync + 'static, A: ToSocketAddrs>(
        timer: Arc<T>,
        peer_addr: A,
        init_seq_num: u32,
    ) -> Result<Stream> {
        let (peer_action_tx, peer_action_rx) = async_channel::unbounded();
        let (user_action_tx, user_action_rx) = async_channel::unbounded();

        let any_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();

        match block_on(UdpSocket::bind(any_addr)) {
            Ok(udp_socket) => {
                let (send_next, receive_next) = block_on(async {
                    UdpSocket::connect(&udp_socket, peer_addr).await.unwrap();
                    Self::client_handshake(&udp_socket, init_seq_num).await
                });

                let local_addr = udp_socket.local_addr().unwrap();

                let join_handle = thread::Builder::new()
                    .name("client".to_string())
                    .spawn(move || {
                        let mut client_connection = ClientConnection::new(
                            udp_socket,
                            peer_action_tx,
                            user_action_rx,
                            send_next,
                            receive_next,
                        );

                        block_on(client_connection.run(timer));
                    })
                    .unwrap();
                let stream = Stream {
                    peer_action_rx,
                    user_action_tx,
                    shutdown_complete: Either::Left(join_handle),
                    shutdown_sent: false,
                    read_timeout: None,
                    last_data: None,
                    local_addr,
                };
                Ok(stream)
            }
            Err(err) => Err(err),
        }
    }

    async fn client_handshake(
        udp_socket: &UdpSocket,
        init_seq_num: u32,
    ) -> (u32, u32) {
        // Send SYN
        let send_next = init_seq_num;
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

    pub fn shutdown(&mut self) {
        self.shutdown_sent = true;
        block_on(self.user_action_tx.send(UserAction::Shutdown)).unwrap();
    }

    // TODO: If no FIN received, a timeout should be used to see that
    // not getting stuck. Have a set_shutdown_timeout function. Wait for EOF
    // on recv channel. If timeout, return. If error closed, return.
    pub fn wait_shutdown_complete(self) {
        match self.shutdown_complete {
            Either::Left(join_handle) => join_handle.join().unwrap(),
            Either::Right(shutdown_complete_rx) => {
                block_on(shutdown_complete_rx.recv()).unwrap()
            }
        }
    }

    pub fn close(self) {
        block_on(self.user_action_tx.send(UserAction::Close)).unwrap();
        self.wait_shutdown_complete();
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        assert!(buf.len() > 0);
        if self.shutdown_sent {
            Err(Error::new(ErrorKind::NotConnected, "Not connected"))
        } else {
            block_on(
                self.user_action_tx.send(UserAction::SendData(buf.to_vec())),
            )
            .unwrap();
            Ok(buf.len())
        }
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let data = match self.last_data.take() {
            Some(data) => Ok(data),
            None => block_on(self.read_peer_action_channel()),
        };

        match data {
            Ok(mut data) => {
                let data_len = data.len();
                let len = std::cmp::min(buf.len(), data_len);
                // TODO: Solve the horrible inefficiency
                for i in 0..len {
                    buf[i] = data[i];
                }

                if data_len > len {
                    let rest = data.split_off(len);
                    self.last_data = Some(rest);
                }

                Ok(len)
            }
            Err(err) => Err(err),
        }
    }

    async fn read_peer_action_channel(&mut self) -> Result<Vec<u8>> {
        let read_future = self.peer_action_rx.recv();
        let timeout_result = match self.read_timeout {
            Some(duration) => future::timeout(duration, read_future).await,
            None => Ok(read_future.await),
        };
        match timeout_result {
            Ok(channel_result) => match channel_result {
                Ok(result) => match result {
                    PeerAction::Data(data) => Ok(data),
                    PeerAction::EOF => Ok(Vec::new()),
                },
                Err(async_channel::RecvError) => {
                    todo!()
                }
            },
            Err(future::TimeoutError { .. }) => {
                Err(Error::new(ErrorKind::WouldBlock, "Would block"))
            }
        }
    }

    pub fn set_read_timeout(&mut self, dur: Option<Duration>) {
        self.read_timeout = dur;
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.local_addr)
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        Stream::read(self, buf)
    }
}

//////////////////////////////////////////////////////////////////
// Server
//////////////////////////////////////////////////////////////////

struct Server<T> {
    udp_socket: Arc<UdpSocket>,
    accept_tx: Sender<(Stream, SocketAddr)>,
    custom_accept_data: Option<Vec<CustomAcceptData<T>>>,
    connections: BTreeMap<SocketAddr, Sender<Segment>>,
}

struct ServerConnection {
    pub connection: Connection,
    pub socket_send_rx: Receiver<Segment>,
    pub socket_receive_tx: Sender<Segment>,
}

impl<T: Timer> Server<T> {
    pub fn new(
        udp_socket: Arc<UdpSocket>,
        accept_tx: Sender<(Stream, SocketAddr)>,
        custom_accept_data: Option<Vec<CustomAcceptData<T>>>,
    ) -> Self {
        Self {
            udp_socket,
            accept_tx,
            custom_accept_data,
            connections: BTreeMap::new(),
        }
    }

    pub async fn server_loop(
        &mut self,
        udp_socket: Arc<UdpSocket>,
        user_action_rx: &Receiver<UserAction>,
    ) {
        let future_recv_socket = Box::pin(Self::recv_socket(&udp_socket));
        let future_recv_user_action =
            Box::pin(Self::recv_user_action(user_action_rx));

        // TODO: Garbage collect old futures
        let mut futures: Vec<
            Pin<Box<dyn futures::Future<Output = ServerSelectResult>>>,
        > = vec![future_recv_socket, future_recv_user_action];

        loop {
            let (result, _index, rest) = select_all(futures).await;
            futures = rest;

            match result {
                ServerSelectResult::RecvSocket(segment, recv_addr) => {
                    futures.push(Box::pin(Self::recv_socket(&udp_socket)));

                    match self.handle_received_segment(segment, recv_addr).await
                    {
                        (
                            true,
                            Some((
                                mut connection,
                                socket_send_rx,
                                shutdown_complete_tx,
                                timer,
                            )),
                        ) => {
                            futures.push(Box::pin(async move {
                                match timer {
                                    Some(timer) => {
                                        connection.run(timer).await;
                                    }
                                    None => {
                                        connection
                                            .run(Arc::new(PlainTimer {}))
                                            .await;
                                    }
                                };

                                shutdown_complete_tx.send(()).await.unwrap();
                                ServerSelectResult::RecvUserAction(None)
                            }));

                            let udp_socket2 = Arc::clone(&udp_socket);
                            futures.push(Box::pin(async move {
                                loop {
                                    match socket_send_rx.recv().await {
                                        Ok((segment, to)) => {
                                            udp_socket2.send_to(&segment.encode(),
                                                        to).await.unwrap();
                                        },
                                        Err(_) => {
                                            return ServerSelectResult::RecvUserAction(None)
                                        }
                                    }
                                }
                            }));
                        }
                        (true, None) => {}
                        (false, _) => {
                            return;
                        }
                    }
                }
                ServerSelectResult::RecvUserAction(user_action) => {
                    match user_action {
                        Some(user_action) => {
                            match user_action {
                                UserAction::Shutdown => {
                                    // TODO: Shut down all connections
                                    return;
                                }
                                _ => {}
                            }
                        }
                        None => {}
                    }
                }
            }
        }
    }

    async fn recv_socket(udp_socket: &Arc<UdpSocket>) -> ServerSelectResult {
        let mut buf = [0; 4096];
        let (amt, recv_addr) = udp_socket.recv_from(&mut buf).await.unwrap();
        ServerSelectResult::RecvSocket(
            Segment::decode(&buf[0..amt]).unwrap(),
            recv_addr,
        )
    }

    async fn recv_user_action(
        user_action_rx: &Receiver<UserAction>,
    ) -> ServerSelectResult {
        let result = match user_action_rx.recv().await {
            Ok(user_action) => Some(user_action),
            Err(_) => None,
        };
        ServerSelectResult::RecvUserAction(result)
    }

    async fn handle_received_segment(
        &mut self,
        segment: Segment,
        recv_addr: SocketAddr,
        // TODO: Make return look better
    ) -> (
        bool,
        Option<(
            Connection,
            Receiver<(Segment, SocketAddr)>,
            Sender<()>,
            Option<Arc<T>>,
        )>,
    ) {
        // println!("handle_received_segment {:?}", segment);
        match segment.kind() {
            Syn => (true, Some(self.handle_syn(segment, recv_addr).await)),
            _ => {
                let socket_receive_tx =
                    self.connections.get(&recv_addr).unwrap();
                match socket_receive_tx.send(segment).await {
                    Ok(_) => (true, None),
                    Err(_) => (false, None),
                }
            }
        }
    }

    async fn handle_syn(
        &mut self,
        segment: Segment,
        recv_addr: SocketAddr,
    ) -> (
        Connection,
        Receiver<(Segment, SocketAddr)>,
        Sender<()>,
        Option<Arc<T>>,
    ) {
        // println!("{:?}", "handle syn");

        let (mut send_next, timer) = match &mut self.custom_accept_data {
            None => (rand::random(), None),
            Some(custom_accept_data) => {
                let data = custom_accept_data.remove(0);
                (data.init_seq_num, Some(data.timer))
            }
        };

        let receive_next = segment.seq_num() + 1;
        let syn_ack = Segment::new_empty(SynAck, send_next, receive_next);
        send_next += 1;

        let encoded_syn_ack = Segment::encode(&syn_ack);

        self.udp_socket
            .send_to(&encoded_syn_ack, recv_addr)
            .await
            .unwrap();

        // println!("accept done");

        let (socket_send_tx, socket_send_rx) = async_channel::unbounded();
        let (socket_receive_tx, socket_receive_rx) = async_channel::unbounded();

        let (peer_action_tx, peer_action_rx) = async_channel::unbounded();
        let (user_action_tx, user_action_rx) = async_channel::unbounded();

        let (shutdown_complete_tx, shutdown_complete_rx) =
            async_channel::bounded(1);

        let connection = Connection::new(
            socket_send_tx,
            socket_receive_rx,
            recv_addr,
            peer_action_tx,
            user_action_rx,
            send_next,
            receive_next,
        );

        let server_stream = Stream {
            peer_action_rx,
            user_action_tx,
            shutdown_complete: Either::Right(shutdown_complete_rx),
            shutdown_sent: false,
            read_timeout: None,
            last_data: None,
            local_addr: self.udp_socket.local_addr().unwrap(),
        };

        assert!(self
            .connections
            .insert(recv_addr, socket_receive_tx)
            .is_none());

        self.accept_tx
            .send((server_stream, recv_addr))
            .await
            .unwrap();

        // TODO: Handle duplicate syn etc
        // TODO: Only one shared socket_send_rx

        (connection, socket_send_rx, shutdown_complete_tx, timer)
    }
}

//////////////////////////////////////////////////////////////////
// Client
//////////////////////////////////////////////////////////////////

struct ClientConnection {
    udp_socket: Arc<UdpSocket>,
    socket_send_rx: Arc<Receiver<(Segment, SocketAddr)>>,
    socket_receive_tx: Arc<Sender<Segment>>,
    connection: Connection,
}

impl ClientConnection {
    pub fn new(
        udp_socket: UdpSocket,
        peer_action_tx: Sender<PeerAction>,
        user_action_rx: Receiver<UserAction>,
        send_next: u32,
        receive_next: u32,
    ) -> Self {
        let (socket_send_tx, socket_send_rx) = async_channel::unbounded();
        let (socket_receive_tx, socket_receive_rx) = async_channel::unbounded();

        let connection = Connection::new(
            socket_send_tx,
            socket_receive_rx,
            udp_socket.peer_addr().unwrap(),
            peer_action_tx,
            user_action_rx,
            send_next,
            receive_next,
        );

        Self {
            udp_socket: Arc::new(udp_socket),
            socket_send_rx: Arc::new(socket_send_rx),
            socket_receive_tx: Arc::new(socket_receive_tx),
            connection,
        }
    }

    pub async fn run<T: Timer>(&mut self, timer: Arc<T>) {
        let udp_socket = Arc::clone(&self.udp_socket);
        let future_recv_socket = Self::recv_socket(&udp_socket).fuse();

        let socket_send_rx = Arc::clone(&self.socket_send_rx);
        let future_recv_socket_send_rx = socket_send_rx.recv().fuse();
        let future_connection = self.connection.run(timer).fuse();

        pin_mut!(
            future_recv_socket,
            future_recv_socket_send_rx,
            future_connection
        );

        loop {
            select! {
                segment = future_recv_socket => {
                    // println!("\n\n recv  {:?}   \n\n\n\n", segment);
                    self.socket_receive_tx.send(segment).await.unwrap();
                    future_recv_socket.set(Self::recv_socket(&udp_socket).fuse());
                },

                segment = future_recv_socket_send_rx => {
                    match segment {
                        Ok((segment, _to)) => {
                            // println!("\n\n send  {:?}   \n\n\n\n", segment);
                            let encoded_segment = segment.encode();
                            udp_socket.send(&encoded_segment).await.unwrap();
                            future_recv_socket_send_rx.set(socket_send_rx.recv().fuse())
                        },
                        Err(_) => {
                            // println!("\n\n  {:?}   \n\n\n\n", "None");
                            return
                        }
                    }
                },

                _ = future_connection => {
                    return;
                }
            }
        }
    }

    async fn recv_socket(udp_socket: &UdpSocket) -> Segment {
        let peer_addr = udp_socket.peer_addr().unwrap();
        let mut buf = [0; 4096];
        let (amt, recv_addr) = udp_socket.recv_from(&mut buf).await.unwrap();
        assert_eq!(peer_addr, recv_addr);
        Segment::decode(&buf[0..amt]).unwrap()
    }
}

//////////////////////////////////////////////////////////////////
// Public structs and interface
//////////////////////////////////////////////////////////////////

struct Connection {
    // Socket
    socket_send_tx: Sender<(Segment, SocketAddr)>,
    socket_receive_rx: Arc<Receiver<Segment>>,
    peer_addr: SocketAddr,

    // Channels
    peer_action_tx: Sender<PeerAction>,
    user_action_rx: Arc<Receiver<UserAction>>,

    // State
    timer_running: bool,
    send_next: u32,
    receive_next: u32,
    fin_sent: bool,
    fin_received: bool,
    send_buffer: Vec<Segment>,
    recv_buffer: BTreeMap<u32, Segment>,
}

impl Connection {
    pub fn new(
        // Socket
        socket_send_tx: Sender<(Segment, SocketAddr)>,
        socket_receive_rx: Receiver<Segment>,
        peer_addr: SocketAddr,

        // Channels
        peer_action_tx: Sender<PeerAction>,
        user_action_rx: Receiver<UserAction>,

        // State
        send_next: u32,
        receive_next: u32,
    ) -> Self {
        Self {
            // Socket
            socket_send_tx,
            socket_receive_rx: Arc::new(socket_receive_rx),
            peer_addr,

            // Channels
            peer_action_tx,
            user_action_rx: Arc::new(user_action_rx),

            // State
            timer_running: false,
            send_next,
            receive_next,
            fin_sent: false,
            fin_received: false,
            send_buffer: Vec::new(),
            recv_buffer: BTreeMap::new(),
        }
    }

    pub async fn run<T: Timer>(&mut self, timer: Arc<T>) {
        let socket_receive_rx = Arc::clone(&self.socket_receive_rx);
        let future_recv_socket_receive = socket_receive_rx.recv().fuse();

        let user_action_rx = Arc::clone(&self.user_action_rx);
        let future_recv_user_action = user_action_rx.recv().fuse();

        let future_timeout = Self::timeout(&timer, true).fuse();

        pin_mut!(
            future_recv_socket_receive,
            future_recv_user_action,
            future_timeout
        );

        loop {
            select! {
                segment = future_recv_socket_receive => {
                    future_recv_socket_receive.set(socket_receive_rx.recv().fuse());
                    if !self.handle_received_segment(segment.unwrap()).await {
                        return;
                    }
                },

                user_action = future_recv_user_action => {
                    future_recv_user_action.set(user_action_rx.recv().fuse());
                    if !self.handle_received_user_action(user_action).await {
                        return;
                    }
                },

                _ = future_timeout => {
                    future_timeout.set(Self::timeout(&timer, false).fuse());

                    self.handle_retransmit().await;
                }
            };
            let buffer_is_empty = self.send_buffer.is_empty();

            if buffer_is_empty && self.timer_running {
                future_timeout.set(Self::timeout(&timer, true).fuse());
                self.timer_running = false;
            } else if !buffer_is_empty && !self.timer_running {
                future_timeout.set(Self::timeout(&timer, false).fuse());
                self.timer_running = true;
            }

            if self.send_buffer.is_empty() && self.fin_sent && self.fin_received
            {
                // TODO: This sleep is needed because once FIN from tc
                // received, Connection returns. But still need
                // to schedule the send of the ACK to that FIN.
                async_std::task::sleep(Duration::from_millis(1)).await;
                return;
            }
        }
    }

    async fn handle_received_segment(&mut self, segment: Segment) -> bool {
        // println!("handle {:?}", segment);
        let ack_num = segment.ack_num();
        let seq_num = segment.seq_num();
        let len = segment.data().len() as u32;
        let kind = segment.kind();

        // TODO: Reject invalid segments instead
        // The segment shouldn't ack something not sent
        assert!(self.send_next >= segment.ack_num());

        if self.fin_received {
            // If FIN has been received, the peer shouldn't send anything
            // new
            assert!(self.receive_next >= seq_num);
        }

        assert!(kind == Fin || kind == Ack);
        if kind == Ack && len == 0 {
            // Do nothing for a pure ACK
            // TODO: Test off by one
        } else if seq_num < self.receive_next {
            // Ignore if too old
        } else {
            self.add_to_recv_buffer(segment);
        }

        let channel_closed = self.deliver_data_to_application().await;

        self.removed_acked_segments(ack_num);

        let ack_needed = len > 0 || kind == Fin;
        if ack_needed {
            self.send_ack().await;
        }

        !channel_closed
    }

    fn removed_acked_segments(&mut self, ack_num: u32) {
        // println!("buffer before: {:?}", self.buffer);
        // println!("ack_num: {:?}", ack_num);
        while self.send_buffer.len() > 0 {
            let first_unacked_segment = &self.send_buffer[0];
            let virtual_len = match first_unacked_segment.kind() {
                Ack => first_unacked_segment.data().len(),
                Fin => 1,
                _ => panic!("Should not have other kinds in this buffer"),
            };

            // Why >= and not >?

            // RFC 793 explicitely says "A segment on the retransmission queue is
            // fully acknowledged if the sum of its sequence number and length is
            // less or equal than the acknowledgment value in the incoming segment."

            // Another way to understand it is that seq_num is the number/index of
            // the first byte in the segment.  So if seq_num=3 and len=2, it means
            // the segment contains byte number 3 and 4, and the next byte to send
            // is 5 (=3+2), which matches the meaning of ack_num=5 "the next byte I
            // expect from you is number 5".
            if ack_num >= first_unacked_segment.seq_num() + virtual_len as u32 {
                self.send_buffer.remove(0);
            } else {
                break;
            }
        }
        // println!("buffer after: {:?}", self.buffer);
    }

    fn add_to_recv_buffer(&mut self, segment: Segment) {
        self.recv_buffer.insert(segment.seq_num(), segment);
        if self.recv_buffer.len() > MAXIMUM_RECV_BUFFER_SIZE {
            self.recv_buffer.pop_last();
        }
    }

    async fn deliver_data_to_application(&mut self) -> bool {
        loop {
            if let Some((seq_num, first_segment)) = self.recv_buffer.pop_first()
            {
                assert_eq!(seq_num, first_segment.seq_num());
                let len = first_segment.data().len() as u32;
                assert!(seq_num >= self.receive_next);
                if seq_num == self.receive_next {
                    if first_segment.kind() == Fin {
                        assert!(len == 0);
                        assert!(!self.fin_received);
                        self.fin_received = true;
                        self.receive_next += 1;

                        match self.peer_action_tx.send(PeerAction::EOF).await {
                            Ok(()) => (),
                            Err(_) => return true,
                        }
                    } else {
                        assert!(len > 0);
                        assert!(first_segment.kind() == Ack);

                        self.receive_next += len;
                        let data = first_segment.to_data();
                        match self
                            .peer_action_tx
                            .send(PeerAction::Data(data))
                            .await
                        {
                            Ok(()) => (),
                            Err(_) => return true,
                        }
                    }
                } else {
                    self.recv_buffer.insert(seq_num, first_segment);
                    return false;
                }
            } else {
                return false;
            }
        }
    }

    async fn send_ack(&self) {
        let ack = Segment::new_empty(Ack, self.send_next, self.receive_next);
        self.send_segment(&ack).await;
    }

    async fn handle_received_user_action(
        &mut self,
        user_action: core::result::Result<UserAction, async_channel::RecvError>,
    ) -> bool {
        match user_action {
            Ok(UserAction::Shutdown) => {
                let seg =
                    Segment::new_empty(Fin, self.send_next, self.receive_next);

                self.send_segment(&seg).await;

                self.send_next += 1;
                self.send_buffer.push(seg);

                self.fin_sent = true;
            }
            Ok(UserAction::SendData(data)) => {
                for chunk in data.chunks(MAXIMUM_SEGMENT_SIZE as usize) {
                    let seg = Segment::new(
                        Ack,
                        self.send_next,
                        self.receive_next,
                        &chunk,
                    );

                    self.send_segment(&seg).await;

                    self.send_next += chunk.len() as u32;
                    self.send_buffer.push(seg);
                }
            }
            Ok(UserAction::Close) => {
                return false;
            }
            Ok(UserAction::Accept) => {
                panic!("Accept for client")
            }
            Err(_) => {
                return false;
            }
        }

        return true;
    }

    async fn send_segment(&self, segment: &Segment) {
        self.socket_send_tx
            .send((segment.clone(), self.peer_addr))
            .await
            .unwrap();
    }

    async fn timeout<T: Timer>(timer: &Arc<T>, forever: bool) {
        if forever {
            timer.sleep(SleepDuration::Forever).await;
            panic!("Forever sleep returned");
        } else {
            timer
                .sleep(SleepDuration::Finite(RETRANSMISSION_TIMER))
                .await;
        }
    }

    async fn handle_retransmit(&self) {
        for segment in &self.send_buffer {
            segment.set_ack_num(self.receive_next);
            // TODO: Add a test for the line above
            // println!("Sendingg {:?}", segment);
            self.send_segment(&segment).await;
        }
    }
}
