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
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Listener> {
        match UdpSocket::bind(addr) {
            Ok(udp_socket) => {
                let listener = Listener { udp_socket };
                Ok(listener)
            }
            Err(err) => Err(err),
        }
    }

    pub fn accept(&self) -> Result<(Stream, SocketAddr)> {
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
        let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();

        match UdpSocket::bind(local_addr) {
            Ok(udp_socket) => {
                UdpSocket::connect(&udp_socket, peer_addr).unwrap();

                udp_socket.send("syn".as_bytes());

                let stream = ClientStream { udp_socket };

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

    fn accept<A: ToSocketAddrs>(peer_addr: A) -> Result<Stream> {
        let local_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();

        match UdpSocket::bind(local_addr) {
            Ok(udp_socket) => {
                UdpSocket::connect(&udp_socket, peer_addr).unwrap();

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
}
