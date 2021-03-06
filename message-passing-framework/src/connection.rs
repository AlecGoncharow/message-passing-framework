use crate::message::{Message, MessageKind};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::*;
use tokio::time::{sleep, Duration};

pub struct Connection<T: MessageKind> {
    messages_in: Arc<Mutex<VecDeque<Message<T>>>>,
    messages_out: Arc<Mutex<VecDeque<Message<T>>>>,

    is_connected: Arc<Mutex<bool>>,
    pub peer_addr: Option<std::net::SocketAddr>,

    read_stream: Option<ReadHalf<tokio::net::TcpStream>>,
    write_stream: Option<WriteHalf<tokio::net::TcpStream>>,
}

impl<T: MessageKind> Connection<T> {
    pub fn new(messages_in: Arc<Mutex<VecDeque<Message<T>>>>) -> Self {
        let messages_out = Arc::new(Mutex::new(VecDeque::new()));

        Self {
            messages_in,
            messages_out,
            peer_addr: None,
            is_connected: Arc::new(Mutex::new(false)),
            write_stream: None,
            read_stream: None,
        }
    }

    pub fn from_stream(
        messages_in: Arc<Mutex<VecDeque<Message<T>>>>,
        stream: tokio::net::TcpStream,
    ) -> Self {
        let messages_out = Arc::new(Mutex::new(VecDeque::new()));
        let peer_addr = Some(stream.peer_addr().unwrap());

        let (read_stream, write_stream) = tokio::io::split(stream);

        Self {
            messages_in,
            messages_out,
            peer_addr,
            is_connected: Arc::new(Mutex::new(true)),
            write_stream: Some(write_stream),
            read_stream: Some(read_stream),
        }
    }

    pub async fn connect_to_server(
        &mut self,
        addr: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send>> {
        match TcpStream::connect(addr).await {
            Ok(stream) => {
                let peer_addr = Some(stream.peer_addr().unwrap());
                self.peer_addr = peer_addr;
                let (read_stream, write_stream) = tokio::io::split(stream);
                self.read_stream = Some(read_stream);
                self.write_stream = Some(write_stream);
                *self.is_connected.lock() = true;
            }
            Err(e) => return Err(Box::new(e)),
        }
        Ok(())
    }

    pub fn start_read_loop(&mut self) {
        if let Some(mut stream) = self.read_stream.take() {
            let messages_in = self.messages_in.clone();
            let is_connected = Arc::clone(&self.is_connected);
            tokio::spawn(async move {
                let mut buf = [0; 1024];

                loop {
                    if !*is_connected.lock() {
                        eprintln!("[Connection] connection closed but still alive?");
                        return;
                    }

                    let byte_count = match stream.read(&mut buf).await {
                        Ok(n) if n == 0 => return,
                        Ok(n) => n,
                        Err(e) => {
                            eprintln!("[Connection] failed to read from socket; err = {:?}", e);
                            match e.kind() {
                                std::io::ErrorKind::BrokenPipe => {
                                    *is_connected.lock() = false;
                                }
                                _ => {
                                    unimplemented!()
                                }
                            }

                            return;
                        }
                    };
                    let msg: Message<T> = Message::from(&buf[0..byte_count]);
                    messages_in.lock().push_back(msg);
                }
            });
        }
    }

    pub fn start_write_loop(&mut self) {
        if let Some(mut stream) = self.write_stream.take() {
            let messages_out = Arc::clone(&self.messages_out);
            let is_connected = Arc::clone(&self.is_connected);
            let peer_addr = self.peer_addr.unwrap().clone();
            tokio::spawn(async move {
                loop {
                    if !*is_connected.lock() {
                        eprintln!("[Connection] connection closed but still alive?");
                        return;
                    }

                    if messages_out.lock().len() > 0 {
                        let next = {
                            let mut write = messages_out.lock();
                            write.pop_front()
                        };
                        if let Some(msg) = next {
                            let bytes: Vec<u8> = Vec::from(msg);

                            if let Err(e) = stream.write(&bytes).await {
                                eprintln!(
                                    "[Connection] failed to write to socket; addr:{:?} err = {:?}",
                                    peer_addr, e
                                );

                                match e.kind() {
                                    std::io::ErrorKind::BrokenPipe => {
                                        *is_connected.lock() = false;
                                    }
                                    _ => {
                                        unimplemented!()
                                    }
                                }

                                return;
                            }
                        }
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            });
        }
    }

    pub async fn disconnect(&mut self) {
        todo!()
    }

    pub fn is_connected(&self) -> bool {
        *self.is_connected.lock()
    }

    pub fn send(&mut self, msg: Message<T>) {
        self.messages_out.lock().push_back(msg);
    }
}
