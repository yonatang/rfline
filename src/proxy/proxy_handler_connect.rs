use std::thread;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, copy};
use tokio::net::TcpStream;
use tokio::{join, spawn};
use crate::proxy::proxy_handler::Target;
use crate::proxy::server::{PResult};

pub async fn handle(target: Target, local_stream: TcpStream) -> PResult<()> {
    let (local_read, mut local_write) = local_stream.into_split();

    let mut local_buf_reader = BufReader::new(local_read);
    let mut line = String::new();
    loop {
        line.clear();
        let read = local_buf_reader.read_line(&mut line).await?;
        if read == 0 {
            log::debug!("Reached EOF from local");
            return Ok(());
        }
        log::debug!("Header [{}]",line.trim_end());
        if line == "\r\n" {
            break;
        }
    }
    log::debug!("Connecting to remote {}:{}",target.host,target.port);
    let remote_stream = TcpStream::connect(format!("{}:{}", target.host, target.port)).await?;
    log::debug!("Remote connected");
    let (mut remote_read, mut remote_write) = remote_stream.into_split();


    let local_write_handler: tokio::task::JoinHandle<std::io::Result<()>> = spawn(async move {
        log::debug!("Sending Connection Established");
        local_write.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n").await?;
        local_write.flush().await?;

        log::debug!("Reading from remote in {:?}",thread::current().id());
        let copied = copy(&mut remote_read, &mut local_write).await?;
        log::debug!("Done reading {copied} from remote");

        Ok(())
    });
    let remote_write_handler: tokio::task::JoinHandle<std::io::Result<()>> = spawn(async move {
        log::debug!("Reading from local in {:?}",thread::current().id());
        copy(&mut local_buf_reader, &mut remote_write).await?;

        Ok(())
    });

    log::debug!("Starting to transfer request");
    let (v1, v2) = join!(local_write_handler,remote_write_handler);
    log::debug!("Request completed");


    let _ = v1??;
    let _ = v2??;
    Ok(())
}