#[cfg(test)]
mod tests;

use std::io::Result;
use std::net::*;
use std::str;
use std::thread;

pub struct Listener {
    udp_socket: UdpSocket,
}

pub struct Stream {
    stream_inner: StreamInner,
}

enum StreamInner {
    Server(ServerStream),
    Client(ClientStream),
}

struct ServerStream {
    udp_socket: UdpSocket,
}

struct ClientStream {
    udp_socket: UdpSocket,
}

impl Listener {
    pub fn bind<A: ToSocketAddrs>(local_addr: A) -> Result<Listener> {
        match UdpSocket::bind(local_addr) {
            Ok(udp_socket) => {
                println!("Listener started");
                let listener = Listener { udp_socket };
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
            let stream = Stream::accept(peer_addr).unwrap();
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

        match UdpSocket::bind(local_addr) {
            Ok(udp_socket) => {
                udp_socket.send_to("syn".as_bytes(), peer_addr).unwrap();
                println!("syn sent");

                let mut buf = [0; 4096];
                let amt = udp_socket.recv(&mut buf).unwrap();
                let string = str::from_utf8(&buf[0..amt]).unwrap();

                if string == "ack" {
                    println!("got ack");
                    let stream = ClientStream { udp_socket };
                    Ok(Self::pack_client_stream(stream))
                } else {
                    println!("got non-ack");
                    unimplemented!()
                }
            }
            Err(err) => Err(err),
        }
    }

    fn pack_client_stream(client_stream: ClientStream) -> Stream {
        Stream {
            stream_inner: StreamInner::Client(client_stream),
        }
    }

    fn accept<A: ToSocketAddrs>(peer_addr: A) -> Result<Stream> {
        let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
        match UdpSocket::bind(local_addr) {
            Ok(udp_socket) => {
                UdpSocket::connect(&udp_socket, peer_addr).unwrap();

                udp_socket.send("ack".as_bytes()).unwrap();

                let stream = ServerStream { udp_socket };

                Ok(Self::pack_server_stream(stream))
            }
            Err(err) => Err(err),
        }
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
        unimplemented!()
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        unimplemented!()
    }
}

impl ServerStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        unimplemented!()
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        unimplemented!()
    }
}
