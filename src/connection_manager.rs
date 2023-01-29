use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
};

use s2n_quic::stream::SendStream;
use tokio::sync::Mutex;

pub type ConnectionManagerMember = Arc<Mutex<SendStream>>;

fn new_connection_manager_member(stream: SendStream) -> ConnectionManagerMember {
    Arc::new(Mutex::new(stream))
}

#[derive(Debug)]
pub struct ConnectionManager {
    send_streams: Mutex<HashMap<String, ConnectionManagerMember>>,
}

impl ConnectionManager {
    pub fn new() -> ConnectionManager {
        ConnectionManager {
            send_streams: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_stream(
        &self,
        target: &String,
        stream: SendStream,
    ) -> Result<(), ConnectionManagerMember> {
        let mut conns = self.send_streams.lock().await;
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
        let mut conns = self.send_streams.lock().await;
        let result = conns.remove(target);
        match result {
            Some(_) => Ok(()),
            None => Err(()),
        }
    }

    pub async fn get_stream(&self, target: &String) -> Result<ConnectionManagerMember, ()> {
        let conns = self.send_streams.lock().await;
        let result = conns.get(target);
        match result {
            Some(v) => Ok(v.clone()),
            None => Err(()),
        }
    }

    pub async fn has_stream(&self, target: &String) -> bool {
        let conns = self.send_streams.lock().await;
        return conns.contains_key(target);
    }
}
