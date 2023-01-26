use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use bytes::Bytes;
use s2n_quic::{
    stream::{ReceiveStream, SendStream},
    Connection,
};
use tokio::{
    sync::{mpsc::UnboundedSender, Mutex},
    task::JoinHandle,
};

use crate::{
    join_handles, me,
    messages::{Message, MessageContent},
    parse,
};

pub type ConnectionManagerMember = Arc<Mutex<SendStream>>;

fn new_connection_manager_member(stream: SendStream) -> ConnectionManagerMember {
    Arc::new(Mutex::new(stream))
}

#[derive(Debug)]
pub struct ConnectionManager {
    conns: Mutex<HashMap<String, ConnectionManagerMember>>,
}

impl ConnectionManager {
    pub fn new() -> ConnectionManager {
        ConnectionManager {
            conns: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_stream(
        &self,
        target: &String,
        stream: SendStream,
    ) -> Result<(), ConnectionManagerMember> {
        let mut conns = self.conns.lock().await;
        let entry = conns.entry(target.clone());
        match entry {
            Entry::Occupied(o) => Err(o.get().clone()),
            Entry::Vacant(v) => {
                v.insert(new_connection_manager_member(stream));
                Ok(())
            }
        }
    }

    pub async fn invalidate_stream(&self, target: &String) -> Result<(), ()> {
        let mut conns = self.conns.lock().await;
        let result = conns.remove(target);
        match result {
            Some(_) => Ok(()),
            None => Err(()),
        }
    }

    pub async fn get_stream(&self, target: &String) -> Result<ConnectionManagerMember, ()> {
        let conns = self.conns.lock().await;
        let result = conns.get(target);
        match result {
            Some(v) => Ok(v.clone()),
            None => Err(()),
        }
    }

    pub async fn has_stream(&self, target: &String) -> bool {
        let conns = self.conns.lock().await;
        return conns.contains_key(target);
    }
}

pub async fn handle_connection(
    cm: Arc<ConnectionManager>,
    mut connection: Connection,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
) {
    let target = connection.server_name().unwrap().unwrap().to_string();
    let mut handles: Vec<JoinHandle<()>> = Vec::new();
    while let Ok(Some(stream)) = connection.accept_bidirectional_stream().await {
        let (stream_receive, stream_send) = stream.split();

        let res = cm.add_stream(&target, stream_send).await;
        if let Err(e) = res {
            eprintln!("{} add stream error {:?}", me(), e);
        }

        println!("{} listening to {} as server", me(), target);

        let handle = tokio::spawn(listen_and_forward(
            cm.clone(),
            stream_receive,
            tx.clone(),
            target.clone(),
        ));
        handles.push(handle);
    }

    join_handles(handles).await;

    println!("{}: closed connection with {:?}", me(), cm);
    if let Err(_) = cm.invalidate_stream(&target).await {
        eprintln!("{} error invalidate stream for target: {}", me(), target);
    } else {
        eprintln!(
            "{} !!successful!! invalidate stream for target: {}",
            me(),
            target
        );
    }
}

pub async fn listen_and_forward(
    cm: Arc<ConnectionManager>,
    mut stream_receive: ReceiveStream,
    tx: Arc<Mutex<UnboundedSender<Message>>>,
    sender: String,
) {
    let mut x = stream_receive.receive().await;
    while let Ok(data_option) = &x {
        if let None = data_option {
            x = stream_receive.receive().await;
            continue;
        }
        let data = data_option.as_ref().unwrap();
        let message = parse(data);
        if let MessageContent::Invalid = message.content {
            continue;
        }
        if !message.from.eq(&sender) {
            eprintln!(
                "{}",
                format!(
                    "{} message expected from {}, rec {}",
                    me(),
                    &sender,
                    &message.from
                )
            );
        }
        tx.lock().await.send(message).unwrap();

        x = stream_receive.receive().await;
    }

    eprintln!("{} rec err {:?}", me(), x);

    if let Err(_) = cm.invalidate_stream(&sender).await {
        eprintln!(
            "{} LAF error invalidate stream for target: {}",
            me(),
            sender
        );
    } else {
        eprintln!(
            "{} LAF !!successful!! invalidate stream for target: {}",
            me(),
            sender
        );
    }
}

pub async fn send(cm: Arc<ConnectionManager>, message: &Message) {
    let data = serde_json::to_string(message);
    if let Err(_) = data {
        // parse error
        return;
    }
    let message_bytes = Bytes::from(data.unwrap());

    let stream = cm.get_stream(&message.to).await;
    if let Err(_) = stream {
        // error case
        return;
    }
    let stream = stream.unwrap();
    stream.lock().await.send(message_bytes).await.unwrap();
}

pub async fn send_owns(cm: Arc<ConnectionManager>, message: Message) {
    send(cm, &message).await;
}
