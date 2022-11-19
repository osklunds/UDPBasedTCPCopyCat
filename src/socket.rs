#[cfg(test)]
mod tests;

use futures::executor::block_on;
use futures::{future::FutureExt, pin_mut, select};
use std::io::Result;
use std::net::*;
use std::str;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

use crate::segment::Kind::*;
use crate::segment::Segment;

pub struct Listener {
    udp_socket: Arc<UdpSocket>,
}

pub struct Stream {
    stream_inner: StreamInner,
}

enum StreamInner {
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

struct Nums {
    seq_num: u32,
    ack_num: u32,
}

impl Listener {
    pub fn bind<A: ToSocketAddrs>(local_addr: A) -> Result<Listener> {
        match UdpSocket::bind(local_addr) {
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
        let (amt, peer_addr) = self.udp_socket.recv_from(&mut buf).unwrap();

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
        //let peer_addr = to_peer_addr.to_socket_addrs().unwrap().last().unwrap();

        let (read_tx, read_rx) = mpsc::channel();
        let (write_tx, write_rx) = mpsc::channel();

        match UdpSocket::bind(local_addr) {
            Ok(udp_socket) => {
                UdpSocket::connect(&udp_socket, peer_addr).unwrap();
                thread::spawn(move || {
                    Self::client(udp_socket, read_tx, write_rx)
                });
                let stream = ClientStream { read_rx, write_tx };
                Ok(Self::pack_client_stream(stream))
            }
            Err(err) => Err(err),
        }
    }

    fn pack_client_stream(client_stream: ClientStream) -> Stream {
        Stream {
            stream_inner: StreamInner::Client(client_stream),
        }
    }

    fn client(
        udp_socket: UdpSocket,
        read_tx: Sender<Vec<u8>>,
        write_rx: Receiver<Vec<u8>>,
    ) {
        let (seq_num, ack_num) = Self::client_handshake(&udp_socket);

        let nums = Nums { seq_num, ack_num };

        let future = connected_loop(nums, udp_socket, read_tx, write_rx);
        block_on(future);
    }

    fn client_handshake(udp_socket: &UdpSocket) -> (u32, u32) {
        // Send SYN
        let seq_num = rand::random();
        let syn = Segment::new(Syn, seq_num, 0, &vec![]);
        let encoded_syn = Segment::encode(&syn);

        udp_socket.send(&encoded_syn).unwrap();

        // Receive SYN-ACK
        let mut buf = [0; 4096];
        let amt = udp_socket.recv(&mut buf).unwrap();

        let syn_ack = Segment::decode(&buf[0..amt]).unwrap();

        let new_seq_num = seq_num + 1;
        // TODO: Handle error instead
        assert_eq!(new_seq_num, syn_ack.ack_num());
        assert_eq!(SynAck, syn_ack.kind());
        assert_eq!(0, syn_ack.data().len());

        let ack_num = syn_ack.seq_num() + 1;

        // Send ACK
        let ack = Segment::new(Ack, new_seq_num, ack_num, &vec![]);
        let encoded_ack = Segment::encode(&ack);

        udp_socket.send(&encoded_ack).unwrap();

        (new_seq_num, ack_num)
    }

    fn accept<A: ToSocketAddrs>(
        udp_socket: Arc<UdpSocket>,
        to_peer_addr: A,
    ) -> Result<Stream> {
        let peer_addr = to_peer_addr.to_socket_addrs().unwrap().last().unwrap();

        udp_socket.send_to("ack".as_bytes(), peer_addr).unwrap();

        let stream = ServerStream {
            udp_socket,
            peer_addr,
        };

        Ok(Self::pack_server_stream(stream))
    }

    fn pack_server_stream(server_stream: ServerStream) -> Stream {
        Stream {
            stream_inner: StreamInner::Server(server_stream),
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match &mut self.stream_inner {
            StreamInner::Client(client_stream) => {
                ClientStream::write(client_stream, buf)
            }
            StreamInner::Server(server_stream) => {
                ServerStream::write(server_stream, buf)
            }
        }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match &mut self.stream_inner {
            StreamInner::Client(client_stream) => {
                ClientStream::read(client_stream, buf)
            }
            StreamInner::Server(server_stream) => {
                ServerStream::read(server_stream, buf)
            }
        }
    }
}

impl ClientStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.write_tx.send(buf.to_vec()).unwrap();

        Ok(buf.len())
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let data = self.read_rx.recv().unwrap();
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
        self.udp_socket.send_to(buf, self.peer_addr)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        println!("server read from {:?}", self.udp_socket.local_addr());
        self.udp_socket.recv(buf)
    }
}

async fn connected_loop(
    nums: Nums,
    udp_socket: UdpSocket,
    read_tx: Sender<Vec<u8>>,
    write_rx: Receiver<Vec<u8>>,
) {
    let nums_in_mutex = Mutex::new(nums);
    let nums_in_arc = Arc::new(nums_in_mutex);

    let recv_socket_state = RecvSocketState {
        udp_socket: &udp_socket,
        read_tx,
        nums: Arc::clone(&nums_in_arc),
    };
    let recv_write_rx_state = RecvWriteRxState {
        udp_socket: &udp_socket,
        write_rx,
        nums: nums_in_arc,
    };

    let future_recv_socket = recv_socket(recv_socket_state).fuse();
    let future_recv_write_rx = recv_write_rx(recv_write_rx_state).fuse();

    pin_mut!(future_recv_socket, future_recv_write_rx);

    loop {
        select! {
            new_recv_socket_state = future_recv_socket => {
                println!("{:?}", "recv sock");

                future_recv_socket.set(recv_socket(new_recv_socket_state).fuse());
            },
            new_write_rx_state = future_recv_write_rx => {
                println!("{:?}", "recv write");

                future_recv_write_rx.set(recv_write_rx(new_write_rx_state).fuse());
            },
        };
    }
}

struct RecvSocketState<'a> {
    udp_socket: &'a UdpSocket,
    read_tx: Sender<Vec<u8>>,
    nums: Arc<Mutex<Nums>>,
}

async fn recv_socket(state: RecvSocketState<'_>) -> RecvSocketState {
    let peer_addr = state.udp_socket.peer_addr().unwrap();
    let segment = recv_segment(state.udp_socket, peer_addr);

    let mut locked_nums = state.nums.lock().unwrap();

    // The segment shouldn't ack something not sent
    assert!(locked_nums.seq_num >= segment.ack_num());
    // The segment should contain the next expected data
    assert_eq!(locked_nums.ack_num, segment.seq_num());

    let data = segment.to_data();
    let len = data.len() as u32;

    locked_nums.ack_num += len;

    let ack =
        Segment::new(Ack, locked_nums.seq_num, locked_nums.ack_num, &vec![]);
    drop(locked_nums);
    send_segment(state.udp_socket, peer_addr, &ack);

    state.read_tx.send(data).unwrap();
    state
}

struct RecvWriteRxState<'a> {
    udp_socket: &'a UdpSocket,
    write_rx: Receiver<Vec<u8>>,
    nums: Arc<Mutex<Nums>>,
}

async fn recv_write_rx(state: RecvWriteRxState<'_>) -> RecvWriteRxState {
    let data = state.write_rx.recv().unwrap();
    let locked_nums = state.nums.lock().unwrap();
    let seg =
        Segment::new(Ack, locked_nums.seq_num, locked_nums.ack_num, &data);
    drop(locked_nums);

    let peer_addr = state.udp_socket.peer_addr().unwrap();
    send_segment(state.udp_socket, peer_addr, &seg);
    state
}

fn recv_segment(udp_socket: &UdpSocket, peer_addr: SocketAddr) -> Segment {
    let mut buf = [0; 4096];
    let (amt, recv_addr) = udp_socket.recv_from(&mut buf).unwrap();
    assert_eq!(peer_addr, recv_addr);
    Segment::decode(&buf[0..amt]).unwrap()
}

fn send_segment(
    udp_socket: &UdpSocket,
    _peer_addr: SocketAddr,
    segment: &Segment,
) {
    let encoded_seq = segment.encode();
    udp_socket.send(&encoded_seq).unwrap();
}
