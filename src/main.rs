extern crate mio;

use std::slice::from_raw_parts_mut;
use std::str::FromStr;
use std::time::Duration;
use std::net::SocketAddr;
use std::io::{Result, Error, ErrorKind};

use mio::*;
use mio::udp::UdpSocket;

const BUF_SIZE: usize = 1024;

pub struct Resolver {
    token: Token,
    sock: UdpSocket,
    server: SocketAddr,
    receive_buf: Option<Vec<u8>>,
}

fn str2socketaddr(s: &str) -> Result<SocketAddr> {
    SocketAddr::from_str(s).map_err(|e| Error::new(ErrorKind::Other, e))
}

impl Resolver {
    fn new(token: Token, server: &str) -> Result<Resolver> {
        #[cfg(feature = "localhost")]
        let addr = "127.0.0.1:0";
        #[cfg(not(feature = "localhost"))]
        let addr = "0.0.0.0:0";
        let addr = str2socketaddr(addr)?;
        let server = str2socketaddr(server)?;
        let sock = UdpSocket::bind(&addr)?;

        Ok(Resolver {
            token: token,
            sock: sock,
            server: server,
            receive_buf: Some(Vec::with_capacity(BUF_SIZE)),
        })
    }

    fn send_request(&self, data: &[u8]) -> Result<()> {
        if let Some(nwrited) = self.sock.send_to(&data, &self.server)? {
            println!("writed {}/{}", nwrited, data.len());
        } else {
            println!("writed nothing");
        }
        Ok(())
    }

    fn receive_data_into_buf(&mut self) -> Result<()> {
        let mut res = Ok(());
        let mut buf = self.receive_buf.take().unwrap();
        // get writable slice from vec
        let ptr = buf.as_mut_ptr();
        let cap = buf.capacity();
        let buf_slice = unsafe { &mut from_raw_parts_mut(ptr, cap) };
        unsafe {
            buf.set_len(0);
        }

        match self.sock.recv_from(buf_slice) {
            Ok(None) => {}
            Ok(Some((nread, addr))) => {
                unsafe {
                    buf.set_len(nread);
                }
                println!("received data from {:?}", addr);
            }
            Err(e) => res = Err(From::from(e)),
        }
        self.receive_buf = Some(buf);
        res
    }

    fn handle_events(&mut self, poll: &Poll, events: Ready) -> Result<()> {
        if events.is_error() {
            let e = self.sock
                .take_error()?
                .or(Some(Error::new(ErrorKind::Other, "event error")))
                .unwrap();
            let _ = poll.deregister(&self.sock);
            self.register(poll)?;
            Err(e)
        } else {
            self.receive_data_into_buf()?;

            if self.receive_buf.as_ref().unwrap().is_empty() {
                Err(Error::new(ErrorKind::Other, "receive buffer is empty"))
            } else {
                let receive_buf = self.receive_buf.take().unwrap();
                println!("received[{}]: {:?}", receive_buf.len(), receive_buf);
                self.receive_buf = Some(receive_buf);
                Ok(())
            }
        }
    }

    fn do_register(&mut self, poll: &Poll, is_reregister: bool) -> Result<()> {
        let events = Ready::readable();
        #[cfg(not(feature = "oneshot"))]
        let pollopts = PollOpt::edge();
        #[cfg(feature = "oneshot")]
        let pollopts = PollOpt::edge() | PollOpt::oneshot();
        println!("pollopts = {:?}", pollopts);

        if is_reregister {
            poll.reregister(&self.sock, self.token, events, pollopts)
                .map_err(From::from)
        } else {
            poll.register(&self.sock, self.token, events, pollopts)
                .map_err(From::from)
        }
    }

    fn register(&mut self, poll: &Poll) -> Result<()> {
        self.do_register(poll, false)
    }

    #[allow(dead_code)]
    fn reregister(&mut self, poll: &Poll) -> Result<()> {
        self.do_register(poll, true)
    }
}

const TIMEOUT: u64 = 5;
const RESOLVER_TOKEN: Token = Token(0);
const TESTS: &'static [&'static str] = &["hi, guys",
                                         "good morning",
                                         "see you later"];
fn main() {
    let server = "127.0.0.1:9000";
    let poll_timeout = Duration::new(TIMEOUT, 0);
    let mut resolver = Resolver::new(RESOLVER_TOKEN, &server).unwrap();
    let poll = Poll::new().unwrap();
    resolver.register(&poll).unwrap();
    let mut events = Events::with_capacity(1024);
    for hostname in TESTS {
        match query(hostname, &mut resolver, &poll, &mut events, poll_timeout) {
            Err(e) => println!("ERROR: {}", e),
            _ => {}
        }
    }
}

fn query(hostname: &'static str,
         resolver: &mut Resolver,
         poll: &Poll,
         events: &mut Events,
         timeout: Duration) -> Result<()> {
    println!("<--------- {}", hostname);

    let _ = resolver.send_request(hostname.as_bytes())?;

    // TODO: debug
    while poll.poll(events, Some(timeout))? == 0 {
        println!("ERROR: timeout of {}", hostname);
    }

    for event in events.iter() {
        println!("{:?}", event);
        match event.token() {
            RESOLVER_TOKEN => {
                #[cfg(feature = "reregister1")]
                resolver.reregister(&poll)?;
                resolver.handle_events(&poll, event.kind())?;
                #[cfg(feature = "reregister2")]
                resolver.reregister(&poll)?;
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}
