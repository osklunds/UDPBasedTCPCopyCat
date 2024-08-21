#[allow(dead_code)]
mod segment;
#[allow(dead_code, unused_imports)]
mod socket;

use socket::Listener;
use socket::Stream;

use std::env;
use std::net::SocketAddr;
use std::io::Read;
use std::path::Path;
use std::time::{Instant};

const BYTES_PER_MEGABYTE: f64 = 1000_000.0;
const BITS_PER_BYTE: f64 = 8.0;

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
        println!("");

        loop {
            let (mut stream, client_addr) = listener.accept().unwrap();
            println!("Connection from {:?}", client_addr);
            let path = read_to_end(&mut stream);
            let path = std::str::from_utf8(&path).unwrap();
            println!("Sending file at {:?}", path);

            let file = match std::fs::read(path) {
                Ok(file) => file,
                Err(_) => {
                    let msg = "Can't find or open file";
                    println!("{}", msg);
                    msg.as_bytes().to_vec()
                }
            };
            stream.write(&file).unwrap();

            stream.shutdown();
            stream.wait_shutdown_complete();

            println!("");
        }
    } else if mode == "client" {
        let server_addr = &args[2];
        let server_addr: SocketAddr = server_addr.parse().unwrap();

        let mut stream = Stream::connect(server_addr).unwrap();
        println!("Connected to {:?}", server_addr);

        let path = &args[3];
        stream.write(path.as_bytes()).unwrap();
        stream.shutdown();

        let start = Instant::now();
        let file = read_all_chunked(&mut stream);
        let elapsed = start.elapsed().as_secs() as f64;

        stream.wait_shutdown_complete();

        let local_path = Path::new(&path);
        let local_path = local_path.file_name().unwrap();
        let mut local_path = local_path.to_str().unwrap().to_owned();
        local_path.push_str(".download");

        let size_in_bytes = file.len();
        std::fs::write(local_path.clone(), file).unwrap();

        print_speed(elapsed, size_in_bytes);

        println!("Saved file to {:?}", local_path);

    } else {
        panic!("Incorrect mode");
    }
}

fn read_all_chunked(stream: &mut Stream) -> Vec<u8> {
    let mut buf = Vec::new();

    loop {
        let start = Instant::now();
        let chunk = read_chunk(stream);
        let elapsed = start.elapsed().as_millis() as f64 / 1000.0;

        buf.extend_from_slice(&chunk);

        if chunk.len() == 0 {
            return buf;
        } else {
            print_speed(elapsed, chunk.len());
        }
    }
}

fn print_speed(elapsed: f64, size_in_bytes: usize) {
    let size_in_megabytes = (size_in_bytes as f64) / BYTES_PER_MEGABYTE;
    let speed = (size_in_bytes as f64 * BITS_PER_BYTE) / (elapsed * BYTES_PER_MEGABYTE);

    println!("Downloaded {:.2} MB ({} bytes) in {:.2} seconds ({:.2} Mbit/s)",
             size_in_megabytes,
             size_in_bytes,
             elapsed,
             speed
    );
}

fn read_chunk(stream: &mut Stream) -> Vec<u8> {
    let mut buf = Vec::new();

    loop {
        let mut inner = [0; 4096];
        match stream.read(&mut inner).unwrap() {
            0 => return buf,
            n => {
                buf.extend_from_slice(&inner[0..n]);
                if buf.len() > BYTES_PER_MEGABYTE as usize {
                    return buf;
                }
            }
        }
    }
}

fn read_to_end(stream: &mut Stream) -> Vec<u8> {
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).unwrap();
    buf
}
