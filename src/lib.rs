use app::{pingpong::PingPongApp, App};
use bytes::{BufMut, Bytes, BytesMut};
use config::ServerInfo;
use connection_manager::ConnectionManager;
use messages::{Message, MessageContent};
use once_cell::sync::Lazy;
use s2n_quic::{client::Connect, stream::ReceiveStream, Client, Connection, Server};
use std::{
    error::Error,
    net::SocketAddr,
    path::Path,
    sync::Arc,
    time::{Duration, SystemTime},
};
use timer::timer;
use tokio::{
    join,
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender},
        Mutex,
    },
    task::JoinHandle,
};

pub mod app;
pub mod config;
pub mod connection_manager;
pub mod messages;
pub mod timer;

// TODO: handle connection re-establishment
// TODO: pull functions into different modules, try to simplify linkages
// TODO: get rid as much as possible of static vars (DI?)
// TODO: eliminate unsafe

// "messages" on stream are separated by delimiter
static DELIMITER: u8 = 0x0d; // '\r'

pub static APP: Lazy<Mutex<PingPongApp>> = Lazy::new(|| Mutex::new(PingPongApp::new()));

pub static mut ME: String = String::new();

pub fn me() -> &'static String {
    unsafe {
        return &ME;
    };
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
    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    while let Some(connection) = server.accept().await {
        let handle = tokio::spawn(handle_connection(connection, tx.clone()));
        handles.push(handle);
    }

    join_handles(handles).await;

    eprintln!("{}: closing server", me());

    Ok(())
}

pub async fn handle_connection(
    mut connection: Connection,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) {
    let mut handles: Vec<JoinHandle<()>> = Vec::new();
    while let Ok(Some(stream)) = connection.accept_receive_stream().await {
        let handle = tokio::spawn(listen_and_forward(stream, tx.clone()));
        handles.push(handle);
    }

    join_handles(handles).await;
}

fn index_first_byte_equals<'a, T>(bytes: T, value: u8) -> Option<usize>
where
    T: IntoIterator<Item = &'a u8>, // Bytes/BytesMut
{
    let mut i: usize = 0;
    for byte in bytes {
        if *byte == value {
            return Some(i);
        }
        i += 1;
    }
    return None;
}

pub async fn listen_and_forward(
    mut stream_receive: ReceiveStream,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) {
    let mut buf = BytesMut::new();
    while let Ok(data_option) = &stream_receive.receive().await {
        if let None = data_option {
            continue;
        }
        let data = data_option.as_ref().unwrap();
        if buf.is_empty() {
            // TODO: eliminiate this horrendous nested conditionals, it's giving me a headache
            if let Some(idx) = index_first_byte_equals(data, DELIMITER) {
                if idx == data.len() - 1 {
                    if let Some(x) = data.strip_suffix(&[DELIMITER]) {
                        let message = parse(x);
                        if let MessageContent::Invalid = message.content {
                            continue;
                        }
                        tx.lock().await.send(message).unwrap();
                        continue;
                    }
                }
            }
        }
        buf.extend(data);
        while let Some(delim_index) = index_first_byte_equals(&buf, DELIMITER) {
            if delim_index == 0 {
                // not using the return buf;
                drop(buf.split_to(1));
                continue;
            }
            let data = buf.split_to(delim_index + 1);
            if let Some(x) = data.strip_suffix(&[DELIMITER]) {
                let message = parse(x);
                if let MessageContent::Invalid = message.content {
                    continue;
                }
                tx.lock().await.send(message).unwrap();
                continue;
            } else {
                println_safe(format!("error in exp {:?}", data));
            }
        }
    }
    println_safe("exiting listen forward");
}

pub async fn send(cm: Arc<ConnectionManager>, message: &Message) {
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
    // stream.send(message_bytes).await.unwrap();
    // stream.send(Bytes::from("\r")).await.unwrap();
    stream.send(mmb.freeze()).await.unwrap();
    // stream.send(mmb.into()).await.unwrap();
}

pub async fn send_owns(cm: Arc<ConnectionManager>, message: Message) {
    send(cm, &message).await;
}

async fn join_handles(handles: Vec<JoinHandle<()>>) {
    for handle in handles {
        let res = join!(handle);
        if let Err(e) = res.0 {
            eprintln!("{:?}", e);
        }
    }
}

fn parse(data: &[u8]) -> Message {
    let json_parse_result: Result<Message, serde_json::Error> = serde_json::from_slice(&data);
    if let Err(x) = &json_parse_result {
        eprintln!("{:?}, {:?}", x, data);
    }
    if let Ok(msg) = json_parse_result {
        return msg;
    }
    eprintln!("******************");
    eprintln!("INVALID: {:?}", data);
    eprintln!("******************");
    Message {
        content: MessageContent::Invalid,
        from: String::from(""),
        to: String::from(""),
    }
}

pub async fn start_pingers(
    cm: Arc<ConnectionManager>,
    servers: Vec<ServerInfo>,
    retry_delay: u32,
    ca_cert_path: String,
) {
    for server_info in servers {
        tokio::spawn(ping_server(
            cm.clone(),
            ca_cert_path.clone(),
            server_info,
            retry_delay,
        ));
    }
}

async fn ping_server(
    cm: Arc<ConnectionManager>,
    ca_cert_path: String,
    server_info: ServerInfo,
    retry_delay: u32,
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

    let client = Client::builder()
        .with_tls(Path::new(&ca_cert_path))
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
