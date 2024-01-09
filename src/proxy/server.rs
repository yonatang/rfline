use std::error::Error;
use tokio::net::{TcpListener};
use tokio::{join, spawn};
use tokio::task::JoinHandle;
use crate::proxy::proxy_handler;

pub type PResult<T> = Result<T, Box<dyn Error>>;
pub type IOResult<T> = std::io::Result<T>;

pub struct Server {
    pub port: Option<u16>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn new() -> Self {
        Server {
            port: None,
            thread: None,
        }
    }
    pub async fn start(&mut self, port: u16) -> IOResult<()> {
        if self.thread.is_some() {
            panic!("Already running")
        }
        log::debug!("Trying to bind on port {port}");
        let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
        let addr = listener.local_addr()?;
        log::info!("Bound to {addr}");
        println!("Server running on port {}", addr.port());
        self.port = Some(addr.port());
        self.thread = Some(spawn(async move {
            loop {
                let (socket, addr) = listener.accept().await.expect("Error accepting connection");
                log::debug!("New request from {}",addr);
                let _ = spawn(async move {
                    let r = proxy_handler::handle_connection(socket).await;
                    match r {
                        Err(e) => log::debug!("Error {:?}",e),
                        Ok(()) => log::debug!("Request completed successfully")
                    };
                });
            }
        }));
        Ok(())
    }

    pub async fn join(&mut self) {
        if let Some(t) = &mut self.thread {
            let _ = join!(t);
        }
    }
}
