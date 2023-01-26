use app::{
    pingpong::{Ping, PingPongApp},
    App,
};
use bytes::Bytes;
use config::ServerInfo;
use connection_manager::{
    handle_connection, listen_and_forward, send, send_owns, ConnectionManager,
};
use messages::{Message, MessageContent};
use once_cell::sync::Lazy;
use s2n_quic::{client::Connect, Client, Server};
use std::{error::Error, net::SocketAddr, path::Path, sync::Arc, time::Duration};
use timer::timer_async;
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

pub static APP: Lazy<Arc<Mutex<PingPongApp>>> =
    Lazy::new(|| Arc::new(Mutex::new(PingPongApp::new())));

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
            eprintln!("{}: ** unhandled ** {:?}", me(), &message);
            continue;
        }
        let result = APP.lock().await.handle(&message);
        if let Err(e) = result {
            eprintln!("{}: {}", me(), e);
            continue;
        }
        let result = result.unwrap();
        if let None = result {
            continue;
        }
        let (out_message, delay) = result.unwrap();
        if let None = delay {
            send(cm.clone(), &out_message).await;
            continue;
        }
        let delay = delay.unwrap();
        timer_async(send_owns(cm.clone(), out_message), delay);
    }
}

pub async fn serve(
    mut server: Server,
    cm: Arc<ConnectionManager>,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) -> Result<(), Box<dyn Error>> {
    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    while let Some(connection) = server.accept().await {
        let handle = tokio::spawn(handle_connection(cm.clone(), connection, tx.clone()));
        handles.push(handle);
    }

    join_handles(handles).await;

    eprintln!("{}: closing server", me());

    Ok(())
}

async fn join_handles(handles: Vec<JoinHandle<()>>) {
    for handle in handles {
        let res = join!(handle);
        if let Err(e) = res.0 {
            eprintln!("{:?}", e);
        }
    }
}

fn parse(data: &Bytes) -> Message {
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
            cm.clone(),
            client_ptr.clone(),
            server_info,
            retry_delay,
            tx.clone(),
        ));
    }
}

async fn ping_server(
    cm: Arc<ConnectionManager>,
    client: Arc<Mutex<Client>>,
    server_info: ServerInfo,
    retry_delay: u32,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) {
    let ServerInfo {
        addr, server_name, ..
    } = server_info;

    if cm.has_stream(&server_name).await {
        init_ping(cm, &server_name).await;
        return;
    }

    let server_addr: SocketAddr = addr.parse().unwrap();
    let connect_config = Connect::new(server_addr).with_server_name(server_name.clone());
    let mut connection = client.lock().await.connect(connect_config.clone()).await;
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
            init_ping(cm, &server_name).await;
            return;
        }
        let delay: u64 = expo_factor * u64::from(retry_delay);
        expo_factor <<= 1; // double
        tokio::time::sleep(Duration::from_millis(delay)).await;
        connection = client.lock().await.connect(connect_config.clone()).await;
    }
    let mut connection = connection.unwrap();

    connection.keep_alive(true).unwrap();

    let mut stream = connection.open_bidirectional_stream().await;
    expo_factor = 1;
    while let Err(_) = stream {
        eprintln!("{}: failed to make a stream to {}\n{:?}", me(), addr, *cm);
        let delay: u64 = expo_factor * u64::from(retry_delay);
        expo_factor <<= 1; // double
        tokio::time::sleep(Duration::from_millis(delay)).await;
        stream = connection.open_bidirectional_stream().await;
    }
    let stream = stream.unwrap();
    let (stream_receive, stream_send) = stream.split();

    let res = cm.add_stream(&server_name, stream_send).await;
    if let Err(e) = res {
        eprintln!("{} readd stream {:?}", me(), e)
    }

    println!("{} listening to {} as client", me(), server_name);

    tokio::spawn(listen_and_forward(
        cm.clone(),
        stream_receive,
        tx,
        server_name.clone(),
    ));

    init_ping(cm, &server_name).await;
}

async fn init_ping(cm: Arc<ConnectionManager>, server_name: &String) {
    let msg = format!("initial ping from {} to {}", me(), server_name);
    let ping = Ping::new(1, msg);
    let content = MessageContent::Ping(ping);
    let message = Message {
        to: server_name.clone(),
        from: me().clone(),
        content,
    };

    let init_status = APP.lock().await.init_pinger(&server_name);
    if let Err(op_sid) = init_status {
        eprintln!("init pinger failed, op: {:?}", op_sid);
        return;
    }

    println!(
        "{} sending first ping from {} to {}",
        me(),
        message.from,
        message.to
    );

    send(cm, &message).await;
}
