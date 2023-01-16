use bytes::Bytes;
use config::ServerInfo;
use once_cell::sync::Lazy;
use pingpong::{Message, Pong};
use s2n_quic::{
    client::Connect,
    stream::{ReceiveStream, SendStream},
    Client, Connection, Server,
};
use std::{
    collections::HashMap, error::Error, net::SocketAddr, path::Path, sync::Arc, time::Duration,
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use crate::pingpong::{Content, Ping};

pub mod config;
pub mod pingpong;

// wrap each send stream in mutex, don't hold map mutex when sending
static SEND_STREAMS: Lazy<Mutex<HashMap<u64, HashMap<u64, SendStream>>>> = Lazy::new(|| {
    let map: HashMap<u64, HashMap<u64, SendStream>> = HashMap::new();
    Mutex::new(map)
});

pub static mut ME: u16 = 0;

pub async fn handle_messages(mut rx: UnboundedReceiver<Message>) {
    while let Some(message) = rx.recv().await {
        let Message {
            conn_id, stream_id, ..
        } = message;
        let response_bytes = process(message);
        if let Some(response_bytes) = response_bytes {
            send(conn_id, stream_id, response_bytes).await;
        }
    }
}

pub async fn serve(
    mut server: Server,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) -> Result<(), Box<dyn Error>> {
    while let Some(connection) = server.accept().await {
        tokio::spawn(handle_connection(connection, tx.clone()));
    }
    unsafe {
        eprintln!("{}: closing server", ME);
    }
    Ok(())
}

async fn handle_connection(mut connection: Connection, tx: Arc<Mutex<UnboundedSender<Message>>>) {
    let conn_id = connection.id();
    SEND_STREAMS.lock().await.insert(conn_id, HashMap::new());
    while let Ok(Some(stream)) = connection.accept_bidirectional_stream().await {
        tokio::spawn(handle_stream(stream, tx.clone(), conn_id));
    }

    unsafe {
        println!("{}: closed connection with conn_id ({})", ME, conn_id);
    }
}

async fn handle_stream(
    stream: s2n_quic::stream::BidirectionalStream,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
    conn_id: u64,
) {
    let stream_id = stream.id();
    let (stream_receive, stream_send) = stream.split();
    SEND_STREAMS
        .lock()
        .await
        .get_mut(&conn_id)
        .unwrap()
        .insert(stream_id, stream_send);

    listen_and_forward(stream_receive, tx, conn_id, stream_id).await;

    // todo!? clean up stream
}

async fn listen_and_forward(
    mut stream_receive: ReceiveStream,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
    conn_id: u64,
    stream_id: u64,
) {
    while let Ok(Some(data)) = stream_receive.receive().await {
        let content: Content = parse(&data);
        tx.lock()
            .await
            .send(Message {
                conn_id,
                stream_id,
                content,
            })
            .unwrap();
    }
}

fn parse(data: &Bytes) -> Content {
    let ping: Result<Ping, serde_json::Error> = serde_json::from_slice(&data);
    if let Ok(ping) = ping {
        return Content::Ping(ping);
    }
    let pong: Result<Pong, serde_json::Error> = serde_json::from_slice(&data);
    if let Ok(pong) = pong {
        return Content::Pong(pong);
    }
    eprintln!("INVALID: {:?}", data);
    Content::Invalid
}

fn process(message: Message) -> Option<Bytes> {
    let Message {
        conn_id,
        stream_id,
        content,
    } = message;
    match content {
        Content::Ping(ping) => process_ping(ping, conn_id, stream_id),
        Content::Pong(pong) => process_pong(pong, conn_id, stream_id),
        Content::Invalid => process_invlid(),
    }
}

fn process_invlid() -> Option<Bytes> {
    unsafe {
        eprintln!("{}: received an invalid message", ME);
    }
    None
}

fn process_ping(ping: Ping, conn_id: u64, stream_id: u64) -> Option<Bytes> {
    unsafe {
        println!("{}: received {:?}", ME, ping);
    }
    let msg = unsafe { format!("pong from {}", ME) };
    let pong = Pong::new(ping.seqnum, msg);
    let response_data = serde_json::to_string(&pong).unwrap();
    let response_bytes = Bytes::from(response_data);
    unsafe {
        println!(
            "{}: sending pong to ({}, {}) = {:?}",
            ME, conn_id, stream_id, pong
        );
    }
    Some(response_bytes)
}

fn process_pong(pong: Pong, conn_id: u64, stream_id: u64) -> Option<Bytes> {
    unsafe {
        println!("{}: received {:?}", ME, pong);
    }
    if pong.seqnum <= 75 {
        tokio::spawn(send_new_ping(pong, conn_id, stream_id));
    }
    None
}

async fn send_new_ping(pong: Pong, conn_id: u64, stream_id: u64) {
    tokio::time::sleep(Duration::from_secs(5)).await;
    let seqnum = pong.seqnum + 1;
    let msg;
    unsafe {
        msg = format!("ping from {} with seqnum {}", ME, seqnum);
    }
    let ping = Ping::new(seqnum, msg);
    let response_data = serde_json::to_string(&ping).unwrap();
    let response_bytes = Bytes::from(response_data);
    unsafe {
        println!(
            "{}: sending ping to ({}, {}) = {:?}",
            ME, conn_id, stream_id, ping
        );
    }
    send(conn_id, stream_id, response_bytes).await;
}

async fn send(conn_id: u64, stream_id: u64, response_data: Bytes) {
    let mut conn_map_locked = SEND_STREAMS.lock().await;
    let streams_map = conn_map_locked.get_mut(&conn_id);
    if let None = streams_map {
        unsafe {
            eprintln!(
                "{}: attempt to send with no conn ({}) data: {:?}",
                ME, conn_id, response_data
            );
        }
        return;
    }
    let streams_map = streams_map.unwrap();
    let stream = streams_map.get_mut(&stream_id);
    if let None = stream {
        unsafe {
            eprintln!(
                "{}: attempt to send with no stream (conn: {}, stream: {}) data: {:?}",
                ME, conn_id, stream_id, response_data
            );
        }
        return;
    }
    let stream = stream.unwrap();
    stream.send(response_data).await.unwrap();
}

pub async fn start_pingers(
    servers: Vec<ServerInfo>,
    retry_delay: u32,
    ca_cert_path: String,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) {
    let client = Client::builder()
        .with_tls(Path::new(&ca_cert_path))
        .unwrap()
        .with_io("127.0.0.1:0")
        .unwrap()
        .start()
        .unwrap();
    let client_ptr = Arc::new(Mutex::new(client));
    for server_info in servers {
        tokio::spawn(ping_server(
            client_ptr.clone(),
            server_info,
            retry_delay,
            tx.clone(),
        ));
    }
}

async fn ping_server(
    client: Arc<Mutex<Client>>,
    server_info: ServerInfo,
    retry_delay: u32,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) {
    let ServerInfo {
        addr,
        port,
        server_name,
    } = server_info;
    let server_addr_str = format!("{}:{}", addr, port);
    let server_addr: SocketAddr = server_addr_str.parse().unwrap();
    let connect_config = Connect::new(server_addr).with_server_name(server_name);
    let mut connection = client.lock().await.connect(connect_config.clone()).await;
    let mut expo_factor: u64 = 1;
    while let Err(_) = connection {
        unsafe {
            eprintln!("{}: failed to make a connection to {}", ME, port);
        }
        let delay: u64 = expo_factor * u64::from(retry_delay);
        expo_factor <<= 1; // double
        tokio::time::sleep(Duration::from_millis(delay)).await;
        connection = client.lock().await.connect(connect_config.clone()).await;
    }
    let mut connection = connection.unwrap();
    connection.keep_alive(true).unwrap();

    let conn_id = connection.id();
    let mut stream = connection.open_bidirectional_stream().await;
    expo_factor = 1;
    while let Err(_) = stream {
        unsafe {
            eprintln!("{}: failed to make a stream to {}", ME, port);
        }
        let delay: u64 = expo_factor * u64::from(retry_delay);
        expo_factor <<= 1; // double
        tokio::time::sleep(Duration::from_millis(delay)).await;
        stream = connection.open_bidirectional_stream().await;
    }
    let stream = stream.unwrap();
    let stream_id = stream.id();
    let (stream_receive, stream_send) = stream.split();
    let mut map = HashMap::new();
    map.insert(stream_id, stream_send);
    let mut conn_map = SEND_STREAMS.lock().await;
    conn_map.insert(conn_id, map);
    let stream_send = conn_map
        .get_mut(&conn_id)
        .unwrap()
        .get_mut(&stream_id)
        .unwrap();

    tokio::spawn(listen_and_forward(stream_receive, tx, conn_id, stream_id));

    let msg;
    unsafe {
        msg = format!("initial ping from {} to {}", ME, server_info.port);
    }
    let ping = Ping::new(1, msg);

    let data = Bytes::from(serde_json::to_string(&ping).unwrap());
    stream_send.send(data).await.unwrap();
    unsafe {
        println!("{}: sending ping 1 to {}", ME, server_info.port);
    }
}
