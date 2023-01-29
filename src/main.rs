use px::app::App;
use px::connection_manager::ConnectionManager;
use px::APP;
use s2n_quic::server::Server;
use s2n_quic::stream::Result;
use std::sync::Arc;
use std::{env::args, error::Error, path::Path};
use tokio::sync::{mpsc, Mutex};

use px::config::{Config, ServerConfig, TlsConfigInfo};

use px::messages::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::new(args());
    let ServerConfig {
        addr,
        tls_config_info,
        id,
    } = config.me;
    let TlsConfigInfo {
        cert_path,
        key_path,
        ca_cert_path,
    } = tls_config_info;

    // set global ME var
    unsafe {
        px::ME = id;
    }
    let server = Server::builder()
        .with_tls((Path::new(&cert_path), Path::new(&key_path)))?
        .with_io(&*addr)?
        .start()?;

    // multi sender 1 receiver channel
    let (tx, rx) = mpsc::unbounded_channel::<Message>();
    let tx = Arc::new(Mutex::new(tx));

    let connection_manager = Arc::new(ConnectionManager::new());

    APP.lock().await.initialize(&connection_manager);

    tokio::spawn(px::handle_messages(connection_manager.clone(), rx));

    tokio::spawn(px::start_pingers(
        connection_manager.clone(),
        config.servers,
        config.retry_delay,
        ca_cert_path,
    ));

    // connection_manager
    px::serve(server, tx).await
}
