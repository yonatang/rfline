use std::fmt::{Display, Formatter};
use tokio::io::{AsyncReadExt};
use tokio::net::TcpStream;
use url::Url;
use crate::proxy;
use crate::proxy::server::PResult;

pub async fn handle_connection(mut stream: TcpStream) -> PResult<()> {
    let target = Target::from_stream(&mut stream).await?;
    log::info!("{}",target);

    match target.method.as_str() {
        "CONNECT" => proxy::proxy_handler_connect::handle(target, stream).await,
        _ => proxy::proxy_handler_http::handle(target, stream).await
    }
}

#[derive(Debug)]
pub(crate) struct Target {
    pub method: String,
    pub host: String,
    pub port: u16,
    pub path_query: String,
}

impl Target {
    async fn from_stream(stream: &mut TcpStream) -> PResult<Target> {
        let mut line = String::new();
        loop {
            let ch = stream.read_u8().await? as char;
            line.push(ch);
            if ch == '\n' {
                break;
            }
        }
        log::debug!("First line is {}", line.trim_end());
        Target::from_str(line.as_str())
        // return Ok(Target::from_str(line.as_str()));
    }
    pub fn from_str(line: &str) -> PResult<Target> {
        let mut it = line.split(" ").take(2);
        let method = it.next().expect("err");
        let target_address = it.next().expect("err");


        match method {
            "CONNECT" => {
                let (host, port_string) = target_address.split_once(':').expect("err");
                let port = port_string.parse::<u16>()?;
                Ok(Target {
                    method: method.to_string(),
                    host: host.to_string(),
                    port,
                    path_query: "".to_string(),
                })
            }
            _ => {
                let url = Url::parse(target_address)?;
                let host = String::from(url.host_str().unwrap());
                let port = url.port_or_known_default().unwrap();
                let path_query = if let Some(query) = url.query() {
                    format!("{}?{}", url.path(), query)
                } else {
                    url.path().to_string()
                };
                Ok(Target {
                    method: method.to_string(),
                    host,
                    port,
                    path_query,
                })
            }
        }
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}:{}{}", self.method, self.host, self.port, self.path_query)
    }
}