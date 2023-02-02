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
    pub fn new(from: String, to: String, content: MessageContent) -> Message {
        Message { from, to, content }
    }
}
