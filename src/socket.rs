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
    pub fn connect<A: ToSocketAddrs>(to_peer_addr: A) -> Result<Stream> {
        println!("connect called");
        let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        let peer_addr = to_peer_addr.to_socket_addrs().unwrap().last().unwrap();

        let (_read_tx, read_rx) = mpsc::channel();
        let (write_tx, _write_rx) = mpsc::channel();

        match UdpSocket::bind(local_addr) {
            Ok(udp_socket) => {
                thread::spawn(move || {
                    let syn = Segment::new_syn();
                    let encoded_syn = Segment::encode(&syn);

                    udp_socket.send_to(&encoded_syn, peer_addr).unwrap();
                    println!("syn sent");

                    let mut buf = [0; 4096];
                    let amt = udp_socket.recv(&mut buf).unwrap();
                    let string = str::from_utf8(&buf[0..amt]).unwrap();

                    if string == "ack" {
                        println!("got ack");
                    } else {
                        println!("got non-ack");
                    }

                    loop {}
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
