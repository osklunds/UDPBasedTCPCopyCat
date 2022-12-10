#[cfg(test)]
mod tests;

use async_channel::{Receiver, Sender};
use async_std::net::*;
use futures::executor::block_on;
use futures::lock::Mutex;
use futures::{future::FutureExt, pin_mut, select};
use std::io::Result;
use std::str;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::segment::Kind::*;
use crate::segment::Segment;

pub struct Listener {
    udp_socket: Arc<UdpSocket>,
}

pub struct Stream {
    inner_stream: InnerStream,
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
    write_tx: Sender<Vec<u8>>,
}

struct ConnectedState {
    seq_num: u32,
    ack_num: u32,
    buffer: Vec<Segment>,
}

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
        let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();

        let (read_tx, read_rx) = async_channel::unbounded();
        let (write_tx, write_rx) = async_channel::unbounded();

        match block_on(UdpSocket::bind(local_addr)) {
            Ok(udp_socket) => {
                block_on(UdpSocket::connect(&udp_socket, peer_addr)).unwrap();

                thread::Builder::new()
                    .name("client".to_string())
                    .spawn(move || {
                        block_on(Self::client(udp_socket, read_tx, write_rx))
                    })
                    .unwrap();

                let client_stream = ClientStream { read_rx, write_tx };
                let inner_stream = InnerStream::Client(client_stream);
                let stream = Stream { inner_stream };
                Ok(stream)
            }
            Err(err) => Err(err),
        }
    }

    async fn client(
        udp_socket: UdpSocket,
        read_tx: Sender<Vec<u8>>,
        write_rx: Receiver<Vec<u8>>,
    ) {
        let (seq_num, ack_num) = Self::client_handshake(&udp_socket).await;

        let state = ConnectedState {
            seq_num,
            ack_num,
            buffer: Vec::new(),
        };

        connected_loop(state, udp_socket, read_tx, write_rx).await;
    }

    async fn client_handshake(udp_socket: &UdpSocket) -> (u32, u32) {
        // Send SYN
        let seq_num = rand::random();
        let syn = Segment::new_empty(Syn, seq_num, 0);
        let encoded_syn = Segment::encode(&syn);

        udp_socket.send(&encoded_syn).await.unwrap();

        // Receive SYN-ACK
        let mut buf = [0; 4096];
        let amt = udp_socket.recv(&mut buf).await.unwrap();

        let syn_ack = Segment::decode(&buf[0..amt]).unwrap();

        let new_seq_num = seq_num + 1;
        // TODO: Handle error instead
        assert_eq!(new_seq_num, syn_ack.ack_num());
        assert_eq!(SynAck, syn_ack.kind());
        assert_eq!(0, syn_ack.data().len());

        let ack_num = syn_ack.seq_num() + 1;

        // Send ACK
        let ack = Segment::new_empty(Ack, new_seq_num, ack_num);
        let encoded_ack = Segment::encode(&ack);

        udp_socket.send(&encoded_ack).await.unwrap();

        (new_seq_num, ack_num)
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
}

impl ClientStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        block_on(self.write_tx.send(buf.to_vec())).unwrap();
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

async fn connected_loop(
    connected_state: ConnectedState,
    udp_socket: UdpSocket,
    read_tx: Sender<Vec<u8>>,
    write_rx: Receiver<Vec<u8>>,
) {
    let connected_state_in_mutex = Mutex::new(connected_state);
    let connected_state_in_arc = Arc::new(connected_state_in_mutex);

    let recv_socket_state = RecvSocketState {
        udp_socket: &udp_socket,
        read_tx,
        connected_state: Arc::clone(&connected_state_in_arc),
    };
    let recv_write_rx_state = RecvWriteRxState {
        udp_socket: &udp_socket,
        write_rx,
        connected_state: connected_state_in_arc,
    };

    let future_recv_socket = recv_socket(recv_socket_state).fuse();
    let future_recv_write_rx = recv_write_rx(recv_write_rx_state).fuse();

    pin_mut!(future_recv_socket, future_recv_write_rx);

    loop {
        select! {
            new_recv_socket_state = future_recv_socket => {
                if let Some(new_recv_socket_state) = new_recv_socket_state {
                    let new_future_recv_socket =
                        recv_socket(new_recv_socket_state).fuse();
                    future_recv_socket.set(new_future_recv_socket);
                } else {
                    return;
                }
            },
            new_write_rx_state = future_recv_write_rx => {
                if let Some(new_write_rx_state) = new_write_rx_state {
                    let new_future_recv_write_rx =
                        recv_write_rx(new_write_rx_state).fuse();
                    future_recv_write_rx.set(new_future_recv_write_rx);
                } else {
                    return;
                }
            },
        };
    }
}

struct RecvSocketState<'a> {
    udp_socket: &'a UdpSocket,
    read_tx: Sender<Vec<u8>>,
    connected_state: Arc<Mutex<ConnectedState>>,
}

async fn recv_socket(state: RecvSocketState<'_>) -> Option<RecvSocketState> {
    let peer_addr = state.udp_socket.peer_addr().unwrap();
    let segment = recv_segment(state.udp_socket, peer_addr).await;

    let mut locked_connected_state = state.connected_state.lock().await;

    // TODO: Reject invalid segments instead
    // The segment shouldn't ack something not sent
    assert!(locked_connected_state.seq_num >= segment.ack_num());
    // The segment should contain the next expected data
    assert_eq!(locked_connected_state.ack_num, segment.seq_num());

    let ack_num = segment.ack_num();
    let data = segment.to_data();
    let len = data.len() as u32;

    locked_connected_state.ack_num += len;

    let ack = Segment::new_empty(
        Ack,
        locked_connected_state.seq_num,
        locked_connected_state.ack_num,
    );

    let buffer = &mut locked_connected_state.buffer;
    let mut something_was_acked = false;
    while buffer.len() > 0 {
        let first_unacked_segment = &buffer[0];
        if ack_num
            >= first_unacked_segment.seq_num()
                + first_unacked_segment.data().len() as u32
        {
            buffer.remove(0);
            something_was_acked = true;
        } else {
            if !something_was_acked {
                // First unacked segment whose seq_num wasn't acked
                for seg in buffer {
                    send_segment(state.udp_socket, peer_addr, &seg).await;
                }
            }

            break;
        }
    }

    drop(locked_connected_state);

    if len == 0 {
        Some(state)
    } else {
        send_segment(state.udp_socket, peer_addr, &ack).await;

        match state.read_tx.send(data).await {
            Ok(()) => Some(state),
            Err(_) => None,
        }
    }
}

struct RecvWriteRxState<'a> {
    udp_socket: &'a UdpSocket,
    write_rx: Receiver<Vec<u8>>,
    connected_state: Arc<Mutex<ConnectedState>>,
}

async fn recv_write_rx(
    state: RecvWriteRxState<'_>,
) -> Option<RecvWriteRxState> {
    match state.write_rx.recv().await {
        Ok(data) => {
            let mut locked_connected_state = state.connected_state.lock().await;
            let seg = Segment::new(
                Ack,
                locked_connected_state.seq_num,
                locked_connected_state.ack_num,
                &data,
            );
            locked_connected_state.seq_num += data.len() as u32;
            locked_connected_state.buffer.push(seg.clone());
            drop(locked_connected_state);

            let peer_addr = state.udp_socket.peer_addr().unwrap();
            send_segment(state.udp_socket, peer_addr, &seg).await;
            Some(state)
        }
        Err(_) => None,
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
