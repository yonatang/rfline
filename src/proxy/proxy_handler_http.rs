use std::{io, thread};
use std::ops::DerefMut;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncBufReadExt, AsyncWriteExt, BufReader, copy};
use tokio::net::TcpStream;
use tokio::{join, spawn};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use crate::proxy::proxy_handler::Target;
use crate::proxy::server::PResult;

pub async fn handle(target: Target, local_stream: TcpStream) -> PResult<()> {
    let (local_read, local_write) = local_stream.into_split();
    let reader = Arc::new(Mutex::new(BufReader::new(local_read)));
    let local_write = Arc::new(Mutex::new(local_write));
    let mut current_target = target;

    loop {
        // TODO reuse connection if possible
        log::debug!("Connecting to remote {}:{}",current_target.host,current_target.port);
        let remote_stream = TcpStream::connect(format!("{}:{}", current_target.host, current_target.port)).await?;
        log::debug!("Remote connected");
        let (mut remote_read, mut remote_write) = remote_stream.into_split();

        let req = format!("{} {} HTTP/1.1\r\n", current_target.method, current_target.path_query);

        remote_write.write_all(req.as_bytes()).await?;

        let reader = Arc::clone(&reader);
        let remote_write_handler: JoinHandle<io::Result<Option<String>>> = spawn(async move {
            let mut reader = reader.lock().await;

            log::debug!("Reading from local in {:?}",thread::current().id());
            let mut line = String::new();
            let mut content_length = 0;
            loop {
                line.clear();

                let size = reader.read_line(&mut line).await?;
                if size <= 0 {
                    log::debug!("Reached EOF from local");
                    remote_write.shutdown().await?;
                    return Ok::<Option<String>, io::Error>(None);
                }
                log::debug!("Header [{}]",line.trim_end());
                if let Some((header_name, val)) = line.split_once(':') {
                    if header_name.eq_ignore_ascii_case("content-length") {
                        content_length = val.trim().parse().unwrap_or(content_length);
                    }
                }
                remote_write.write_all(line.as_bytes()).await?;
                remote_write.flush().await?;
                if line == "\r\n" {
                    break;
                }
            }
            log::debug!("Request body length: {content_length}");
            if content_length > 0 {
                let reader_borrow = reader.deref_mut();
                let mut content_reader = reader_borrow.take(content_length);
                copy(&mut content_reader, &mut remote_write).await?;
            }

            line.clear();
            let size = reader.read_line(&mut line).await?;
            if size == 0 {
                log::debug!("Reached EOF from local");
                remote_write.shutdown().await?;
                return Ok(None);
            }
            log::debug!("Next request is {}", line.trim_end());
            return Ok(Some(line));
        });

        let local_write = Arc::clone(&local_write);
        let local_write_handler: JoinHandle<std::io::Result<()>> = spawn(async move {
            let mut binding = local_write.lock().await;
            let mut local_write = binding.deref_mut();

            log::debug!("Reading from remote in {:?}",thread::current().id());
            let copied = copy(&mut remote_read, &mut local_write).await?;
            log::debug!("Done reading {copied} from remote");
            Ok(())
        });
        log::debug!("Starting to transfer request");
        let (remote_result, _) = join!(remote_write_handler,local_write_handler);

        log::debug!("Request completed");

        if let Some(line) = remote_result?? {
            log::debug!("Next request {}", line.trim_end());
            current_target = Target::from_str(line.as_str())?;
            log::info!("{}",current_target);
        } else {
            log::debug!("No more requests");
            return Ok(());
        }
    }
}