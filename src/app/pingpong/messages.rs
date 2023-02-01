use serde::{Deserialize, Serialize};

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Ping {
    pub seqnum: u64,
    pub message: String,
}

impl Ping {
    pub fn new(seqnum: u64, message: String) -> Ping {
        Ping { seqnum, message }
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Pong {
    pub seqnum: u64,
    pub message: String,
}

impl Pong {
    pub fn new(seqnum: u64, msg: String) -> Pong {
        Pong {
            seqnum,
            message: msg,
        }
    }
}
