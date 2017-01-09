extern crate mio;

use std::str::FromStr;
use std::time::Duration;
use std::net::SocketAddr;

use mio::*;
use mio::udp::UdpSocket;

const SERVER_ADDR: &'static str = "127.0.0.1:9000";
const BUF_SIZE: usize = 1024;
const UDP_TOKEN: Token = Token(0);
const TESTS: &'static [&'static str] = &["hi, guys",
                                         "good morning",
                                         "see you later"];

fn main() {
    let timeout = Duration::new(5, 0);
    let server_addr = SocketAddr::from_str(SERVER_ADDR).unwrap();
    let sock = UdpSocket::bind(&SocketAddr::from_str("127.0.0.1:0").unwrap()).unwrap();
    let poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(1024);
    let pollopts = if cfg!(feature = "level") {
        PollOpt::level()
    } else {
        if cfg!(feature = "oneshot") {
            PollOpt::edge() | PollOpt::oneshot()
        } else {
            PollOpt::edge()
        }
    };

    poll.register(&sock, UDP_TOKEN, Ready::readable(), pollopts).unwrap();

    // send to echo server
    for test in TESTS {
        match sock.send_to(test.as_bytes(), &server_addr) {
            Ok(Some(nwrite)) => println!("writed {}/{}", nwrite, test.len()),
            Ok(None) => println!("writed nothing"),
            Err(e) => println!("send_to error {}", e),
        }
    }

    let mut cnt = TESTS.len();
    let mut failed_cnt = TESTS.len() * 2;
    while cnt > 0 && failed_cnt > 0 {
        match poll.poll(&mut events, Some(timeout)) {
            Ok(0) => println!("poll nothing or timeout"),
            Err(e) => println!("poll error: {}", e),
            _ => {
                failed_cnt += 1;
            }
        }
        failed_cnt -= 1;

        for event in events.iter() {
            println!("{:?}", event);
            match event.token() {
                UDP_TOKEN => {
                    if event.kind().is_error() {
                        let e = sock.take_error().unwrap();
                        println!("event error: {:?}", e);
                        return;
                    }

                    // read response from echo server
                    if event.kind().is_readable() {
                        let buf = &mut [0u8; BUF_SIZE];
                        match sock.recv_from(buf) {
                            Ok(None) => println!("recv_from blocked"),
                            Ok(Some((nread, addr))) => {
                                println!("{:?} [{}] => {:?}", addr, nread, &buf[..nread]);
                            }
                            Err(e) => println!("recv_from error: {}", e),
                        }
                    }

                    #[cfg(feature = "reregister")]
                    poll.reregister(&sock, UDP_TOKEN, Ready::readable(), pollopts).unwrap();
                    cnt -= 1;
                }
                _ => unreachable!(),
            }
        }
    }
}
