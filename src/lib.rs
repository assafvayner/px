use app::{pingpong::PingPongApp, App};
use bytes::{BufMut, Bytes, BytesMut};
use config::{Config, ServerInfo};
use connection_manager::ConnectionManager;
use message_parser::MessageParser;
use messages::Message;
use once_cell::sync::Lazy;
use s2n_quic::{stream::ReceiveStream, Connection, Server};
use std::{env::args, error::Error, sync::Arc, time::SystemTime};
use timer::timer;
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

pub mod app;
pub mod config;
pub mod connection_manager;
pub mod message_parser;
pub mod messages;
pub mod timer;

// TODO: pull functions into different modules, try to simplify linkages

// "messages" on stream are separated by delimiter
static DELIMITER: u8 = 0x0d; // '\r'

#[cfg(feature = "pingpong")]
pub static APP: Lazy<Mutex<PingPongApp>> = Lazy::new(|| Mutex::new(PingPongApp::new()));

pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::new(args()));

pub static ME: Lazy<&'static String> = Lazy::new(|| &CONFIG.me.id);

pub static CM: Lazy<ConnectionManager> = Lazy::new(|| ConnectionManager::new());

/// helper to get reference to ME string
pub fn me() -> &'static String {
    return &ME;
}

pub async fn handle_messages(mut rx: UnboundedReceiver<Message>) {
    while let Some(message) = rx.recv().await {
        let Message { content, .. } = &message;
        if !PingPongApp::handles(content) {
            #[cfg(debug_assertions)]
            println_safe(format!("{}: ** unhandled ** {:?}", me(), &message));
            continue;
        }
        let result = APP.lock().await.handle(&message);
        if let Err(e) = result {
            #[cfg(debug_assertions)]
            println_safe(format!("{}: {}", me(), e));
            continue;
        }
        let result = result.unwrap();

        // handle app result
        if let None = result {
            // nothing to send
            continue;
        }
        let (out_message, delay) = result.unwrap();
        if let None = delay {
            // send immediately
            send(&out_message).await;
            continue;
        }
        // send with delay
        let delay = delay.unwrap();
        timer(send_owns(out_message), delay);
    }
}

pub async fn serve(
    mut server: Server,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) -> Result<(), Box<dyn Error>> {
    while let Some(connection) = server.accept().await {
        tokio::spawn(handle_connection(connection, tx.clone()));
    }
    #[cfg(debug_assertions)]
    println_safe(format!("{}: closing server", me()));
    Ok(())
}

pub async fn handle_connection(
    mut connection: Connection,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) {
    while let Ok(Some(stream)) = connection.accept_receive_stream().await {
        listen_and_forward(stream, &tx).await;
        // listen and forward only completes on stream close, only allow 1 stream per connection
        // will only accept another stream after previous stream close.
    }
}

async fn listen_and_forward(
    mut stream_receive: ReceiveStream,
    tx: &Mutex<UnboundedSender<Message>>,
) {
    let mut parser = MessageParser::new();
    while let Ok(Some(data)) = stream_receive.receive().await {
        parser.append_data(&data);
        forward(&mut parser, tx).await;
    }
    // end loop, stream closed (error or Ok(None) from stream.receive())
}

#[inline]
async fn forward<T>(parser: &mut T, tx: &Mutex<UnboundedSender<Message>>)
where
    T: Iterator<Item = Message>,
{
    for message in parser {
        if let Err(e) = tx.lock().await.send(message) {
            #[cfg(debug_assertions)]
            println_safe(format!("forward error: {}", e));
        }
    }
}

pub(crate) async fn send(message: &Message) {
    let data = serde_json::to_string(message);
    if let Err(_) = data {
        // parse error
        return;
    }
    let message_bytes = Bytes::from(data.unwrap());

    let mut mmb = BytesMut::with_capacity(message_bytes.len() + 1);
    mmb.put(message_bytes);
    mmb.put_u8(DELIMITER);

    let stream = CM.get_stream(&message.to).await;
    if let Err(_) = stream {
        // error case
        return;
    }
    let stream = stream.unwrap();
    let mut stream = stream.lock().await;
    stream.send(mmb.freeze()).await.unwrap();
}

pub(crate) async fn send_owns(message: Message) {
    send(&message).await;
}

pub async fn start_send_streams() {
    let ca_cert_path = &CONFIG.me.tls_config_info.ca_cert_path;
    let retry_delay = CONFIG.retry_delay;

    #[cfg(feature = "pingpong")]
    let on_connection_success = init_ping;

    #[cfg(feature = "paxos")]
    let on_connection_success = todo!();

    for server_info in &CONFIG.servers {
        let ServerInfo {
            addr, server_name, ..
        } = server_info;

        tokio::spawn(CM.init_sender(
            addr,
            server_name,
            ca_cert_path,
            retry_delay.into(),
            on_connection_success,
        ));
    }
}

/// function that is used to notify the ping pong app of a new sending connection
#[cfg(feature = "pingpong")]
async fn init_ping(server_name: &String) {
    APP.lock().await.init_pinger(&server_name).await;
}

pub(crate) fn println_safe<'a, T>(what: T)
where
    T: std::fmt::Display,
{
    eprint!(
        "{}",
        format!(
            "{:?} {}: {}\n",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_micros(),
            me(),
            what
        )
    );
}
