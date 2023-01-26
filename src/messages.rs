use serde::{Deserialize, Serialize};

use crate::app::pingpong::{Ping, Pong};

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum MessageContent {
    Invalid,
    Ping(Ping),
    Pong(Pong),
}

impl MessageContent {
    pub fn message_type(&self) -> &'static str {
        match self {
            MessageContent::Invalid => "Invalid",
            MessageContent::Ping(_) => "Ping",
            MessageContent::Pong(_) => "Pong",
        }
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub content: MessageContent,
}

impl Message {
    pub fn new(from: String, to: String, content: MessageContent) -> Message {
        Message { from, to, content }
    }
}

#[derive(Debug, Hash, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct WhoAmINotification {
    pub server_id: String,
}
