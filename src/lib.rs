use app::{pingpong::PingPongApp, App};
use bytes::{BufMut, Bytes, BytesMut};
use config::{Config, ServerInfo};
use connection_manager::ConnectionManager;
use message_parser::MessageParser;
use messages::Message;
use once_cell::sync::Lazy;
use s2n_quic::{client::Connect, stream::ReceiveStream, Client, Connection, Server};
use std::{
    env::args,
    error::Error,
    net::SocketAddr,
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime},
};
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

// TODO: handle connection re-establishment
// TODO: pull functions into different modules, try to simplify linkages
// TODO: get rid as much as possible of static vars (DI?)
// TODO: eliminate unsafe

// "messages" on stream are separated by delimiter
static DELIMITER: u8 = 0x0d; // '\r'

pub static APP: Lazy<Mutex<PingPongApp>> = Lazy::new(|| Mutex::new(PingPongApp::new()));

pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::new(args()));

pub static ME: Lazy<&'static String> = Lazy::new(|| &CONFIG.me.id);

pub fn me() -> &'static String {
    // unsafe {
    return &ME;
    // };
}

pub async fn handle_messages(cm: Arc<ConnectionManager>, mut rx: UnboundedReceiver<Message>) {
    while let Some(message) = rx.recv().await {
        let Message { content, .. } = &message;
        if !PingPongApp::handles(content) {
            println_safe(format!("{}: ** unhandled ** {:?}", me(), &message));
            continue;
        }
        let result = APP.lock().await.handle(&message);
        if let Err(e) = result {
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
            send(cm.clone(), &out_message).await;
            continue;
        }
        // send with delay
        let delay = delay.unwrap();
        timer(send_owns(cm.clone(), out_message), delay);
    }
}

pub async fn serve(
    mut server: Server,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) -> Result<(), Box<dyn Error>> {
    while let Some(connection) = server.accept().await {
        tokio::spawn(handle_connection(connection, tx.clone()));
    }
    eprintln!("{}: closing server", me());
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
            println_safe(format!("forward error: {}", e));
        }
    }
}

pub(crate) async fn send(cm: Arc<ConnectionManager>, message: &Message) {
    let data = serde_json::to_string(message);
    if let Err(_) = data {
        // parse error
        return;
    }
    let message_bytes = Bytes::from(data.unwrap());

    let mut mmb = BytesMut::with_capacity(message_bytes.len() + 1);
    mmb.put(message_bytes);
    mmb.put_u8(DELIMITER);

    let stream = cm.get_stream(&message.to).await;
    if let Err(_) = stream {
        // error case
        return;
    }
    let stream = stream.unwrap();
    let mut stream = stream.lock().await;
    stream.send(mmb.freeze()).await.unwrap();
}

pub(crate) async fn send_owns(cm: Arc<ConnectionManager>, message: Message) {
    send(cm, &message).await;
}

pub async fn start_pingers(
    cm: Arc<ConnectionManager>,
    // servers: Vec<ServerInfo>,
    // retry_delay: u32,
    // ca_cert_path: &'static String,
) {
    for server_info in &CONFIG.servers {
        tokio::spawn(ping_server(
            cm.clone(),
            // ca_cert_path,
            server_info,
            // retry_delay,
        ));
    }
}

async fn ping_server(
    cm: Arc<ConnectionManager>,
    // ca_cert_path: &'static String,
    server_info: &ServerInfo,
    // retry_delay: u32,
) {
    let ServerInfo {
        addr, server_name, ..
    } = server_info;
    println_safe(format!("ping server {}", &server_name));

    if cm.has_stream(&server_name).await {
        eprintln!("case 1");
        init_ping(&server_name).await;
        return;
    }

    let ca_cert_path = &CONFIG.me.tls_config_info.ca_cert_path;
    let retry_delay = CONFIG.retry_delay;

    let client = Client::builder()
        .with_tls(Path::new(ca_cert_path))
        .unwrap()
        .with_io("127.0.0.1:0")
        .unwrap()
        .start()
        .unwrap();

    let server_addr: SocketAddr = addr.parse().unwrap();
    let connect_config = Connect::new(server_addr).with_server_name(server_name.clone());
    let mut connection = client.connect(connect_config.clone()).await;
    let mut expo_factor: u64 = 1;
    while let Err(_) = connection {
        eprintln!(
            "{}: failed to make a connection to {}, {:?}",
            me(),
            addr,
            *cm
        );
        // if failed because has it, just send
        if cm.has_stream(&server_name).await {
            eprintln!("case 2");
            init_ping(&server_name).await;
            return;
        }
        let delay: u64 = expo_factor * u64::from(retry_delay);
        expo_factor <<= 1; // double
        tokio::time::sleep(Duration::from_millis(delay)).await;
        connection = client.connect(connect_config.clone()).await;
    }
    let mut connection = connection.unwrap();

    connection.keep_alive(true).unwrap();

    let mut stream = connection.open_send_stream().await;
    expo_factor = 1;
    while let Err(_) = stream {
        eprintln!("{}: failed to make a stream to {}\n{:?}", me(), addr, *cm);
        let delay: u64 = expo_factor * u64::from(retry_delay);
        expo_factor <<= 1; // double
        tokio::time::sleep(Duration::from_millis(delay)).await;
        stream = connection.open_send_stream().await;
    }
    let stream = stream.unwrap();

    let res = cm.add_stream(&server_name, stream).await;
    if let Err(e) = res {
        eprintln!("{} readd stream {:?}", me(), e)
    }

    init_ping(&server_name).await;
}

#[inline]
async fn init_ping(server_name: &String) {
    let init_status = APP.lock().await.init_pinger(&server_name).await;
    if let Err(op_sid) = init_status {
        eprintln!("init pinger failed, op: {:?}", op_sid);
        return;
    }
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
