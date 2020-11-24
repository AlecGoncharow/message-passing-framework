use crate::connection::Connection;
use crate::message::{Message, MessageKind};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

pub struct ServerInterface<T: MessageKind> {
    port: u16,
    messages_in: Arc<Mutex<VecDeque<Message<T>>>>,
    connections: Arc<Mutex<Vec<Connection<T>>>>,
    #[allow(dead_code)]
    id_counter: usize,
}

impl<T: MessageKind> ServerInterface<T> {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            messages_in: Arc::new(Mutex::new(VecDeque::new())),
            connections: Arc::new(Mutex::new(vec![])),
            id_counter: 10000,
        }
    }

    pub async fn start(&mut self) {
        self.listen_for_connections();
    }

    pub async fn update(&mut self) {
        if self.messages_in.lock().expect("poisoned lock").len() > 0 {
            let mut write = self.messages_in.lock().expect("poisoned lock");
            while let Some(msg) = write.pop_front() {
                println!("[Server] got msg {}", msg);
            }
        }

        /*
                   for connection in self.connections.write().unwrap().iter_mut() {
        //connection.ping().await;
        }
        */
    }

    pub async fn send_to_all(&mut self, msg: Message<T>) {
        for connection in self.connections.lock().unwrap().iter_mut() {
            connection.send(msg.clone()).await;
        }
    }

    pub fn stop(&mut self) {
        todo!()
    }

    pub fn connection_count(&mut self) -> usize {
        self.connections.lock().unwrap().len()
    }

    fn listen_for_connections(&self) {
        let port = self.port;
        let connections = self.connections.clone();
        let messages_in = self.messages_in.clone();

        tokio::spawn(async move {
            let addr = format!("127.0.0.1:{}", port);
            println!("[Server] starting on {}", addr);
            let listener = match TcpListener::bind(addr).await {
                Ok(listener) => listener,
                Err(_) => unimplemented!(),
            };

            loop {
                let (socket, _) = match listener.accept().await {
                    Ok(accept) => accept,
                    Err(_) => unimplemented!(),
                };

                println!("[Server] new client on {:#?}", socket.peer_addr().unwrap());
                let mut connection = Connection::from_stream(messages_in.clone(), socket);
                //connection.ping().await;
                connection.start_read_loop();
                connection.start_write_loop();
                let mut write = connections.lock().expect("poisoned lock");
                write.push(connection);
            }
        });
    }
}
