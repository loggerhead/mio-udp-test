extern crate mio;

use std::str::FromStr;
use std::time::Duration;
use std::net::SocketAddr;
use std::io::{Result, Error, ErrorKind};
use std::collections::VecDeque;

use mio::*;
use mio::udp::UdpSocket;

const BUF_SIZE: usize = 1024;

struct Resolver {
    pub token: Token,
    pub sock: UdpSocket,
    pub echo_server: SocketAddr,
    pub tasks: VecDeque<Vec<u8>>,
}

fn str2socketaddr(s: &str) -> Result<SocketAddr> {
    SocketAddr::from_str(s).map_err(|e| Error::new(ErrorKind::Other, e))
}

impl Resolver {
    fn new(token: Token, echo_server: &str) -> Result<Resolver> {
        #[cfg(feature = "localhost")]
        let addr = "127.0.0.1:0";
        #[cfg(not(feature = "localhost"))]
        let addr = "0.0.0.0:0";
        let addr = str2socketaddr(addr)?;
        let echo_server = str2socketaddr(echo_server)?;
        let sock = UdpSocket::bind(&addr)?;

        Ok(Resolver {
            token: token,
            sock: sock,
            echo_server: echo_server,
            tasks: VecDeque::new(),
        })
    }

    fn add_task(&mut self, _poll: &Poll, data: &[u8]) -> Result<()> {
        self.tasks.push_back(data.to_vec());
        #[cfg(feature = "reregister")]
        self.reregister(_poll, Ready::readable() | Ready::writable())?;
        Ok(())
    }

    fn handle_event(&mut self, _poll: &Poll, event: Ready) -> Result<()> {
        if event.is_error() {
            return Err(self.sock.take_error()?
                       .or(Some(Error::new(ErrorKind::Other, "event error")))
                       .unwrap());
        }

        if event.is_writable() {
            if let Some(data) = self.tasks.pop_front() {
                self.send_request(&data)?;
            }
        }

        if event.is_readable() {
            let data = self.receive_data()?;
            println!("received[{}]: {:?}", data.len(), data);
        }

        #[cfg(feature = "reregister")]
        {
            let interest = if self.tasks.is_empty() {
                Ready::readable()
            } else {
                Ready::readable() | Ready::writable()
            };
            println!("reregistered {:?}", interest);
            self.reregister(_poll, interest)?;
        }
        Ok(())
    }

    fn send_request(&self, data: &[u8]) -> Result<bool> {
        match self.sock.send_to(&data, &self.echo_server) {
            Ok(None) => Err(Error::new(ErrorKind::WouldBlock, "write nothing")),
            Ok(Some(nwrite)) => Ok(nwrite == data.len()),
            Err(e) => Err(e),
        }
    }

    fn receive_data(&mut self) -> Result<Vec<u8>> {
        let mut buf = [0u8; BUF_SIZE];

        match self.sock.recv_from(&mut buf) {
            Ok(None) => Err(Error::new(ErrorKind::WouldBlock, "read nothing")),
            Ok(Some((nread, addr))) => {
                println!("received {} bytes from {:?}", nread, addr);
                Ok(buf[..nread].to_vec())
            }
            Err(e) => Err(e),
        }
    }

    fn do_register(&mut self, poll: &Poll, events: Ready, is_reregister: bool) -> Result<()> {
        let pollopts = if cfg!(feature = "level") {
            PollOpt::level()
        } else {
            if cfg!(feature = "oneshot") {
                PollOpt::edge() | PollOpt::oneshot()
            } else {
                PollOpt::edge()
            }
        };

        if is_reregister {
            poll.reregister(&self.sock, self.token, events, pollopts)
        } else {
            println!("registered ({:?}, {:?})", pollopts, events);
            poll.register(&self.sock, self.token, events, pollopts)
        }
    }

    fn register(&mut self, poll: &Poll, events: Ready) -> Result<()> {
        self.do_register(poll, events, false)
    }

    #[allow(dead_code)]
    fn reregister(&mut self, poll: &Poll, events: Ready) -> Result<()> {
        self.do_register(poll, events, true)
    }
}

const ECHO_SERVER: &'static str = "127.0.0.1:9000";
const TIMEOUT: u64 = 5;
const RESOLVER_TOKEN: Token = Token(0);
const TESTS: &'static [&'static str] = &["hi, guys",
                                         "good morning",
                                         "see you later"];
fn main() {
    let timeout = Duration::new(TIMEOUT, 0);
    let mut resolver = Resolver::new(RESOLVER_TOKEN, ECHO_SERVER).unwrap();
    let poll = Poll::new().unwrap();
    let mut events = Events::with_capacity(1024);
    resolver.register(&poll, Ready::readable()).unwrap();

    for hostname in TESTS {
        if let Err(e) = resolver.add_task(&poll, hostname.as_bytes()) {
            println!("add task failed: {}", e);
        }
    }

    loop {
        match query(&mut resolver, &poll, &mut events, timeout) {
            // Ok(finished) => if finished { break; },
            Err(e) => println!("ERROR: {}", e),
            _ => {},
        }
    }
}

fn query(resolver: &mut Resolver,
         poll: &Poll,
         events: &mut Events,
         timeout: Duration) -> Result<bool> {
    let nevents = poll.poll(events, Some(timeout))?;
    if nevents == 0 {
        println!("poll no events");
    } else {
        for event in events.iter() {
            let token = event.token();
            let kind = event.kind();
            println!("{:?} => {:?}", token, kind);
            if token == RESOLVER_TOKEN {
                resolver.handle_event(poll, kind)?;
            }

            if kind.is_readable() && resolver.tasks.is_empty() {
                return Ok(true)
            }
        }
    }

    Ok(false)
}
