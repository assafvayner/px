use px::CONFIG;
use s2n_quic::server::Server;
use s2n_quic::stream::Result;
use std::sync::Arc;
use std::{error::Error, path::Path};
use tokio::sync::{mpsc, Mutex};

use px::config::{ServerConfig, TlsConfigInfo};

use px::messages::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let ServerConfig {
        addr,
        tls_config_info,
        ..
    } = &CONFIG.me;
    let TlsConfigInfo {
        cert_path,
        key_path,
        ..
    } = tls_config_info;

    let server = Server::builder()
        .with_tls((Path::new(&cert_path), Path::new(&key_path)))?
        .with_io(addr.as_str())?
        .start()?;

    // multi sender 1 receiver channel
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let tx = Arc::new(Mutex::new(tx));

    tokio::spawn(px::handle_messages(rx));

    tokio::spawn(px::start_send_streams());

    px::serve(server, tx).await
}
