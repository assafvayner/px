use s2n_quic::server::Server;
use s2n_quic::stream::Result;
use std::sync::Arc;
use std::{env::args, error::Error, path::Path};
use tokio::sync::{mpsc, Mutex};

use px::config::{Config, ServerConfig, TlsConfigInfo};

use px::pingpong::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new(args());
    let ServerConfig {
        addr,
        port,
        tls_config_info,
        ping,
    } = config.me;
    let TlsConfigInfo {
        cert_path,
        key_path,
        ca_cert_path,
    } = tls_config_info;
    unsafe {
        px::ME = port;
    }
    let server_address = format!("{}:{}", addr, port);
    let server = Server::builder()
        .with_tls((Path::new(&cert_path), Path::new(&key_path)))?
        .with_io(&*server_address)?
        .start()?;
    println!("started server\n{:?}", server);

    // multi sender 1 receiver channel
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let tx = Arc::new(Mutex::new(tx));

    tokio::spawn(px::handle_messages(rx));

    if ping && config.servers.len() > 0 {
        tokio::spawn(px::start_pingers(
            config.servers,
            config.retry_delay,
            ca_cert_path,
            tx.clone(),
        ));
    }

    px::serve(server, tx).await
}
