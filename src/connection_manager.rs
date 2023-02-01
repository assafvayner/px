use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    path::Path,
    sync::Arc,
    time::Duration,
};

use futures::Future;
use s2n_quic::{client::Connect, stream::SendStream, Client};
use tokio::sync::Mutex;

use crate::debug_println;

// TODO: reintro expo backoff on connection re-establishment

pub type ConnectionManagerValue = Arc<Mutex<SendStream>>;

fn new_connection_manager_member(stream: SendStream) -> ConnectionManagerValue {
    Arc::new(Mutex::new(stream))
}

#[derive(Debug)]
pub struct ConnectionManager {
    send_streams: Mutex<HashMap<String, ConnectionManagerValue>>,
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
    ) -> Result<(), ConnectionManagerValue> {
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

    pub async fn get_stream(&self, target: &String) -> Result<ConnectionManagerValue, ()> {
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

    pub async fn init_sender<F, Fut>(
        &self,
        addr: &'static String,
        server_name: &'static String,
        ca_cert_path: &'static String,
        retry_delay: u64,
        on_connection_success: F,
    ) where
        F: Fn(&'static String) -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut client: Client;
        let server_addr: SocketAddr = addr.parse().unwrap();
        let connect_config = Connect::new(server_addr).with_server_name(server_name.as_str());

        loop {
            self.ensure_no_stream(server_name).await;

            client = get_client(ca_cert_path);
            let mut connection = client.connect(connect_config.clone()).await;
            while let Err(e) = connection {
                #[cfg(debug_assertions)]
                debug_println(format!("failed to make a connection to {}, {}", addr, e));
                tokio::time::sleep(Duration::from_millis(retry_delay)).await;
                connection = client.connect(connect_config.clone()).await;
            }
            let mut connection = connection.unwrap();

            connection.keep_alive(true).unwrap();

            let mut stream = connection.open_send_stream().await;
            while let Err(_) = stream {
                #[cfg(debug_assertions)]
                debug_println(format!("failed to open send stream to {}", addr));
                tokio::time::sleep(Duration::from_millis(retry_delay)).await;
                stream = connection.open_send_stream().await;
            }
            let stream = stream.unwrap();

            let res = self.add_stream(server_name, stream).await;
            if let Err(e) = res {
                #[cfg(debug_assertions)]
                debug_println(format!("register stream failed, e: {:?}", e));
            }
            on_connection_success(server_name).await;
            let res = client.wait_idle().await;
            if let Err(e) = res {
                #[cfg(debug_assertions)]
                debug_println(format!("error on idle, {}", e));

                break;
            }
        }
    }

    async fn ensure_no_stream(&self, server_name: &String) {
        if !self.has_stream(server_name).await {
            return;
        }
        let res = self.invalidate_stream(server_name).await;
        if let Err(_) = res {
            #[cfg(debug_assertions)]
            debug_println(format!(
                "register stream failed, server_name {}",
                server_name
            ));
        }
    }
}

/// util get quic client
fn get_client(ca_cert_path: &String) -> Client {
    Client::builder()
        .with_tls(Path::new(ca_cert_path))
        .unwrap()
        .with_io("127.0.0.1:0")
        .unwrap()
        .start()
        .unwrap()
}
