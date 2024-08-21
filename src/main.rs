#[allow(dead_code)]
mod segment;
#[allow(dead_code, unused_imports)]
mod socket;

use socket::Listener;
use socket::Stream;

use std::env;
use std::net::SocketAddr;
use std::io::Read;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mode = &args[1];

    if mode == "server" {
        let port = &args[2];

        let mut all_addr = "0.0.0.0:0".to_owned();
        all_addr.push_str(port);

        let all_addr: SocketAddr = all_addr.parse().unwrap();
        let listener = Listener::bind(all_addr).unwrap();
        println!("Listening to {:?}", listener.local_addr().unwrap());

        loop {
            let (mut stream, _client_addr) = listener.accept().unwrap();
            println!("{:?}", "Got connection");
            let path = read_to_end(&mut stream);
            let path = std::str::from_utf8(&path).unwrap();
            println!("{:?}", path);

            let file = std::fs::read(path).unwrap();
            stream.write(&file).unwrap();

            stream.shutdown();
            stream.wait_shutdown_complete();
        }
    } else if mode == "client" {
        let server_addr = &args[2];
        let server_addr: SocketAddr = server_addr.parse().unwrap();

        let mut stream = Stream::connect(server_addr).unwrap();
        println!("Connected to {:?}", server_addr);

        let path = &args[3];
        stream.write(path.as_bytes()).unwrap();
        stream.shutdown();

        let file = read_to_end(&mut stream);
        println!("{:?}", file);

        stream.wait_shutdown_complete();

        let mut local_path = path.to_owned();
        local_path.push_str(".download");

        std::fs::write(local_path, file).unwrap();

    } else {
        panic!("Incorrect mode");
    }
}

fn read_to_end(stream: &mut Stream) -> Vec<u8> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    buf
}
