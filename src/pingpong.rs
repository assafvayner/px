use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq)]
pub struct Ping {
    ping: u8,
    pub seqnum: u64,
    pub msg: String,
}

impl Ping {
    pub fn new(seqnum: u64, msg: String) -> Ping {
        Ping {
            ping: 0,
            seqnum,
            msg,
        }
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq)]
pub struct Pong {
    pong: u8,
    pub seqnum: u64,
    pub msg: String,
}

impl Pong {
    pub fn new(seqnum: u64, msg: String) -> Pong {
        Pong {
            pong: 0,
            seqnum,
            msg,
        }
    }
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub enum Content {
    Invalid,
    Ping(Ping),
    Pong(Pong),
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct Message {
    pub conn_id: u64,
    pub stream_id: u64,
    pub content: Content,
}
