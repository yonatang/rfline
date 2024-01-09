use std::error::Error;

mod proxy;


#[tokio::main]
async fn main() -> Result<(),Box<dyn Error>> {
    env_logger::init();
    log::info!("Starting server");
    let mut s = proxy::server::Server::new();
    s.start(7878).await?;
    s.join().await;
    Ok(())
}
