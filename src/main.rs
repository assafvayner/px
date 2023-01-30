use px::app::App;
use px::connection_manager::ConnectionManager;
use px::{APP, CONFIG};
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

    // convert &'static String to &'static str
    let addr: &'static str = &*(*addr);

    let server = Server::builder()
        .with_tls((Path::new(&cert_path), Path::new(&key_path)))?
        .with_io(addr)?
        .start()?;

    // multi sender 1 receiver channel
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let tx = Arc::new(Mutex::new(tx));

    let connection_manager = Arc::new(ConnectionManager::new());

    APP.lock().await.initialize(&connection_manager);

    tokio::spawn(px::handle_messages(connection_manager.clone(), rx));

    tokio::spawn(px::start_pingers(connection_manager.clone()));

    // connection_manager
    px::serve(server, tx).await
}
