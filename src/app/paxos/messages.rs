use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{ballot::Ballot, data::Data, log::PaxosLog};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosProposeRequest {
    pub slot: usize,
    pub leader: Ballot,
    pub data: Data,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosProposeReply {
    pub slot: usize,
    pub leader: Ballot,
    pub data: Data,
    pub chosen: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct PaxosKnowledgeStateUpdate {
    pub leader: Ballot,
    pub map: BTreeMap<String, usize>,
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
