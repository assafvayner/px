use crate::messages::{Message, MessageContent};

use self::messages::{
    PaxosKnowledgeStateUpdate, PaxosLeaderElectionReply, PaxosLeaderElectionRequest,
    PaxosLivenessNotification, PaxosProposeReply, PaxosProposeRequest,
};

use super::{App, AppError, AppResult};

pub mod messages;

pub(crate) struct PaxosApp {}

impl App for PaxosApp {
    fn new() -> Self {
        PaxosApp {}
    }

    fn handles(message: &MessageContent) -> bool {
        match message {
            MessageContent::PaxosKnowledgeStateUpdate(_)
            | MessageContent::PaxosLeaderElectionRequest(_)
            | MessageContent::PaxosLeaderElectionReply(_)
            | MessageContent::PaxosLivenessNotification(_)
            | MessageContent::PaxosProposeRequest(_)
            | MessageContent::PaxosProposeReply(_) => true,
            _ => false,
        }
    }

    fn handle(&mut self, message: &Message) -> AppResult {
        let Message { content, from, .. } = message;
        match content {
            MessageContent::PaxosKnowledgeStateUpdate(knowledge_state_update) => {
                self.process_knowledge_state_update(knowledge_state_update, from)
            }
            MessageContent::PaxosLeaderElectionRequest(leader_election_request) => {
                self.process_leader_election_request(leader_election_request, from)
            }
            MessageContent::PaxosLeaderElectionReply(leader_election_reply) => {
                self.process_leader_election_reply(leader_election_reply, from)
            }
            MessageContent::PaxosLivenessNotification(liveness_notification) => {
                self.process_liveness_notification(liveness_notification, from)
            }
            MessageContent::PaxosProposeRequest(propose_request) => {
                self.process_propose_request(propose_request, from)
            }
            MessageContent::PaxosProposeReply(propose_reply) => {
                self.process_propose_reply(propose_reply, from)
            }
            m => Err(AppError {
                error_message: format!("paxos app does not handle {} messages", m.message_type()),
            }),
        }
    }
}

impl PaxosApp {
    fn process_leader_election_request(
        &mut self,
        leader_election_request: &PaxosLeaderElectionRequest,
        from: &String,
    ) -> AppResult {
        todo!()
    }

    fn process_leader_election_reply(
        &mut self,
        leader_election_reply: &PaxosLeaderElectionReply,
        from: &String,
    ) -> AppResult {
        todo!()
    }

    fn process_liveness_notification(
        &mut self,
        liveness_notification: &PaxosLivenessNotification,
        from: &String,
    ) -> AppResult {
        todo!()
    }

    fn process_knowledge_state_update(
        &mut self,
        knowledge_state_update: &PaxosKnowledgeStateUpdate,
        _from: &String,
    ) -> AppResult {
        todo!()
    }

    fn process_propose_request(
        &mut self,
        propose_request: &PaxosProposeRequest,
        from: &String,
    ) -> AppResult {
        todo!()
    }

    fn process_propose_reply(
        &mut self,
        propose_reply: &PaxosProposeReply,
        from: &String,
    ) -> AppResult {
        todo!()
    }
}
