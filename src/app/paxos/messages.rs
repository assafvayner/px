use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Ballot {
    pub leader: String,
    pub number: u64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosProposeRequest {
    pub slot: u64,
    pub leader: Ballot,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosProposeReply {
    pub slot: u64,
    pub leader: Ballot,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub selected: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosKnowledgeStateUpdate {
    pub map: HashMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosLeaderElectionRequest {
    pub ballot: Ballot,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosLeaderElectionReply {
    pub ballot: Ballot,
    // log
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosLivenessNotification {
    pub leader: Ballot,
    // log
}
