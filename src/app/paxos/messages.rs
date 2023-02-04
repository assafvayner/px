use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{ballot::Ballot, log::PaxosLog};

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
    pub leader: Ballot,
    pub map: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosLeaderElectionRequest {
    pub ballot: Ballot,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosLeaderElectionReply {
    pub ballot: Ballot,
    pub log: PaxosLog,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosLivenessNotification {
    pub leader: Ballot,
    pub log: PaxosLog,
}
