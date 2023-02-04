use serde::{Deserialize, Serialize};

use crate::app::{
    paxos::messages::{
        PaxosKnowledgeStateUpdate, PaxosLeaderElectionReply, PaxosLeaderElectionRequest,
        PaxosLivenessNotification, PaxosProposeReply, PaxosProposeRequest,
    },
    pingpong::messages::{Ping, Pong},
};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub enum MessageContent {
    Invalid,

    // pingpong
    Ping(Ping),
    Pong(Pong),

    // paxos
    PaxosKnowledgeStateUpdate(PaxosKnowledgeStateUpdate),
    PaxosLeaderElectionRequest(PaxosLeaderElectionRequest),
    PaxosLeaderElectionReply(PaxosLeaderElectionReply),
    PaxosLivenessNotification(PaxosLivenessNotification),
    PaxosProposeRequest(PaxosProposeRequest),
    PaxosProposeReply(PaxosProposeReply),
}

impl MessageContent {
    pub fn message_type(&self) -> &'static str {
        match self {
            MessageContent::Invalid => "Invalid",

            // ping pong
            MessageContent::Ping(_) => "Ping",
            MessageContent::Pong(_) => "Pong",

            // paxos
            MessageContent::PaxosKnowledgeStateUpdate(_) => "PaxosKnowledgeStateUpdate",
            MessageContent::PaxosLeaderElectionRequest(_) => "PaxosLeaderElection_Request",
            MessageContent::PaxosLeaderElectionReply(_) => "PaxosLeaderElectionReply",
            MessageContent::PaxosLivenessNotification(_) => "PaxosLivenessNotification",
            MessageContent::PaxosProposeRequest(_) => "PaxosProposeRequest",
            MessageContent::PaxosProposeReply(_) => "PaxoseProposeReply",
        }
    }
}

impl std::fmt::Display for MessageContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageContent::Invalid => write!(f, "invalid"),
            MessageContent::Ping(ping) => write!(f, "{:?}", ping),
            MessageContent::Pong(pong) => write!(f, "{:?}", pong),
            MessageContent::PaxosKnowledgeStateUpdate(ksu) => write!(f, "{:?}", ksu),
            MessageContent::PaxosLeaderElectionRequest(le_req) => write!(f, "{:?}", le_req),
            MessageContent::PaxosLeaderElectionReply(le_rep) => write!(f, "{:?}", le_rep),
            MessageContent::PaxosLivenessNotification(ln) => write!(f, "{:?}", ln),
            MessageContent::PaxosProposeRequest(p_req) => write!(f, "{:?}", p_req),
            MessageContent::PaxosProposeReply(p_rep) => write!(f, "{:?}", p_rep),
        }
    }
}

/// Core message structure being transmitted between nodes
/// must contain valid from field when sent as receiving streams
/// are not aware of the sender's identity
/// content is related to applications
///
/// nodes are trusted in this environment
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub content: MessageContent,
}

impl Message {
    pub fn from(from: String, to: String, content: MessageContent) -> Message {
        Message { from, to, content }
    }
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Message(from: {}, to: {} <> {})",
            self.from, self.to, self.content
        )
    }
}
