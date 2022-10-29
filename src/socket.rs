
#[cfg(test)]
mod tests;

use std::net::*;
use std::io::Result;

pub struct Listener {
    udp_socket: UdpSocket
}

pub struct Stream {

}

impl Listener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> Result<Listener> {
        match UdpSocket::bind(addr) {
            Ok(udp_socket) => {
                let listener = Listener {
                    udp_socket
                };
                Ok(listener)
            }
            Err(err) => {
                Err(err)
            }
        }
    }

    pub fn accept(&self) -> Result<(Stream, SocketAddr)> {
        let ip = Ipv4Addr::new(1,2,3,4);
        let port = 123;
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, port));
        Ok((Stream{}, addr))
    }
}


