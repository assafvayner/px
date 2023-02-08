use px::CONFIG;
use s2n_quic::server::Server;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use px::config::{ServerConfig, TlsConfigInfo};

use px::messages::Message;

#[tokio::main]
async fn main() {
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
        .with_tls((Path::new(&cert_path), Path::new(&key_path)))
        .unwrap()
        .with_io(addr.as_str())
        .unwrap()
        .start()
        .unwrap();

    // multi sender 1 receiver channel
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let tx = Arc::new(Mutex::new(tx));

    tokio::spawn(px::handle_messages(rx));

    tokio::spawn(px::start_send_streams());

    tokio::spawn(px::listen_stdin());

    px::serve(server, tx).await
}
