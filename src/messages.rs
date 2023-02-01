use serde::{Deserialize, Serialize};

use crate::app::pingpong::messages::{Ping, Pong};

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

/// Core message structure being transmitted between nodes
/// must contain valid from field when sent as receiving streams
/// are not aware of the sender's identity
/// content is related to applications
/// 
/// nodes are trusted in this environment
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
