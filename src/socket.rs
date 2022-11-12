#[cfg(test)]
mod tests;

use std::io::Result;
use std::net::*;
use std::str;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

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
    read_rx: Receiver<()>,
    write_tx: Sender<()>,
}

struct State {
    seq_num: u32,
    ack_num: u32,
    udp_socket: UdpSocket,
    read_tx: Sender<()>,
    write_rx: Receiver<()>,
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
        println!("connect called");
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
        read_tx: Sender<()>,
        write_rx: Receiver<()>,
    ) {
        let (seq_num, ack_num) = Self::client_handshake(&udp_socket);

        let mut state = State {
            udp_socket,
            seq_num,
            ack_num,
            read_tx,
            write_rx,
        };

        loop {
            connected_loop(&mut state);
        }
    }

    fn client_handshake(udp_socket: &UdpSocket) -> (u32, u32) {
        let seq_num = rand::random();
        let syn = Segment::new(true, false, false, seq_num, 0, &vec![]);
        let encoded_syn = Segment::encode(&syn);

        udp_socket.send(&encoded_syn).unwrap();

        let mut buf = [0; 4096];
        let amt = udp_socket.recv(&mut buf).unwrap();

        let syn_ack = Segment::decode(&buf[0..amt]).unwrap();

        let new_seq_num = seq_num + 1;
        // TODO: Handle error instead
        assert_eq!(new_seq_num, syn_ack.ack_num());
        assert_eq!(true, syn_ack.syn());
        assert_eq!(true, syn_ack.ack());
        assert_eq!(false, syn_ack.fin());
        assert_eq!(0, syn_ack.data().len());

        let ack_num = syn_ack.seq_num() + 1;

        let ack =
            Segment::new(false, true, false, new_seq_num, ack_num, &vec![]);
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
    fn write(&mut self, _buf: &[u8]) -> Result<usize> {
        unimplemented!()
        // println!("client write to {:?}", self.peer_addr);
        // self.udp_socket.send_to(buf, self.peer_addr)
    }

    fn read(&mut self, _buf: &mut [u8]) -> Result<usize> {
        unimplemented!()
        // self.udp_socket.recv(buf)
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

fn connected_loop(state: &mut State) {
    let udp_socket = &state.udp_socket;
    let peer_addr = udp_socket.peer_addr().unwrap();
    let segment = recv_segment(udp_socket, peer_addr);

    // The segment shouldn't ack something not sent
    assert!(state.seq_num >= segment.ack_num());
    // The segment should contain the next expected data
    assert_eq!(state.ack_num, segment.seq_num());

    let data = segment.data();
    let len = data.len() as u32;

    state.ack_num += len;

    let ack =
        Segment::new(false, true, false, state.seq_num, state.ack_num, &vec![]);
    send_segment(&udp_socket, peer_addr, &ack);
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
