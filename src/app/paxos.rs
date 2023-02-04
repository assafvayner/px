use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_recursion::async_recursion;

use crate::messages::{Message, MessageContent};
use crate::{debug_println, me, send, timer, APP};

use self::ballot::Ballot;
use self::log::PaxosLog;

use self::messages::{
    PaxosKnowledgeStateUpdate, PaxosLeaderElectionReply, PaxosLeaderElectionRequest,
    PaxosLivenessNotification, PaxosProposeReply, PaxosProposeRequest,
};

use super::{App, AppError, AppResult};

pub mod ballot;
pub mod log;
pub mod messages;

static LIVENESS_NOTIFICATION_DELAY: u64 = 5000;
static LIVENESS_REQUIREMENT_DELAY: u64 = 20000;
static LEADER_ELECTION_TIMER: u64 = 5000;

// next TODO: monitor leader heartbeats
// next TODO: how to trigger leader elections

pub struct PaxosApp {
    init: bool,
    election_cycle: u64,
    log: PaxosLog,
    leader: Ballot,
    servers: BTreeSet<String>,
    election: Option<BTreeSet<String>>,
    history: BTreeMap<String, u64>,
    last_leader_liveness_notification_timestamp: SystemTime,
}

impl PaxosApp {
    pub async fn init(&mut self) {
        if self.init {
            return;
        }
        self.init = true;
        self.init_leader_election().await;
    }

    pub fn new<T: Iterator<Item = String>>(servers: T) -> Self {
        let mut history = BTreeMap::new();
        let mut servers_set: BTreeSet<String> = BTreeSet::new();
        for server in servers {
            history.insert(server.clone(), 0);
            servers_set.insert(server);
        }
        PaxosApp {
            init: false,
            election_cycle: 0,
            log: PaxosLog::default(),
            leader: Ballot::default(),
            servers: servers_set,
            election: None,
            history,
            last_leader_liveness_notification_timestamp: SystemTime::from(UNIX_EPOCH),
        }
    }

    pub fn is_leader(&self) -> bool {
        return self.election.eq(&None) && me().eq(&self.leader.leader);
    }

    async fn init_leader_election(&mut self) {
        // debug_println(std::backtrace::Backtrace::force_capture());
        self.election_cycle = self.leader.number + 1;
        self.leader = Ballot::new(me().clone(), self.election_cycle);
        let leader_election_request = PaxosLeaderElectionRequest {
            ballot: self.leader.clone(),
        };
        let mut message = Message::from(
            me().clone(),
            String::new(),
            MessageContent::PaxosLeaderElectionRequest(leader_election_request),
        );
        let mut election = BTreeSet::new();
        election.insert(me().clone());
        self.election = Some(election);
        broadcast(self.servers.iter(), &mut message).await;
        timer(
            leader_election_check(message, self.leader.clone()),
            Duration::from_millis(LEADER_ELECTION_TIMER),
        );
    }

    fn process_leader_election_request(
        &mut self,
        leader_election_request: &PaxosLeaderElectionRequest,
        from: &String,
    ) -> AppResult {
        if !self.servers.contains(from) {
            return Err(AppError {
                error_message: format!("server {} is unknown", from),
            });
        }
        let candidate_ballot = &leader_election_request.ballot;
        if !from.eq(&candidate_ballot.leader) {
            return Err(AppError {
                error_message: format!(
                    "server {} attempts to elect other {} as leader",
                    from, &candidate_ballot.leader
                ),
            });
        }

        // if this is a better leader accept it as such
        if candidate_ballot > &self.leader {
            self.election = None;
            self.leader = candidate_ballot.clone();
            timer(
                check_leader_live(self.leader.clone()),
                Duration::from_millis(LIVENESS_REQUIREMENT_DELAY),
            );
        }

        // send leader election reply with self.{ballot, log}
        // this either affirms choosing new leader or lets sender know
        // there is a better ballot
        let leader_election_reply = Message::from(
            me().clone(),
            from.clone(),
            MessageContent::PaxosLeaderElectionReply(PaxosLeaderElectionReply {
                ballot: self.leader.clone(),
                log: self.log.clone(),
            }),
        );
        return Ok(Some((leader_election_reply, None)));
    }

    fn process_leader_election_reply(
        &mut self,
        leader_election_reply: &PaxosLeaderElectionReply,
        from: &String,
    ) -> AppResult {
        if !self.servers.contains(from) {
            return Err(AppError {
                error_message: format!("server {} is unknown", from),
            });
        }

        if let None = self.election {
            return Ok(None);
        }

        let ballot = &leader_election_reply.ballot;
        let ord = self.leader.cmp(ballot);

        if let Ordering::Greater = ord {
            return Ok(None);
        }

        if let Ordering::Less = ord {
            // if older ballot with me as leader, however this is guarenteed to not occur since then my leader would be greater or equal
            // if me().eq(&ballot.leader) {
            //     return Ok(None);
            // }
            self.election = None;
            self.leader = ballot.clone();
            timer(
                check_leader_live(self.leader.clone()),
                Duration::from_millis(LIVENESS_REQUIREMENT_DELAY),
            );

            let new_leader_election_reply = Message::from(
                me().clone(),
                from.clone(),
                MessageContent::PaxosLeaderElectionReply(PaxosLeaderElectionReply {
                    ballot: self.leader.clone(),
                    log: self.log.clone(),
                }),
            );
            return Ok(Some((new_leader_election_reply, None)));
        }

        // merge logs tbd
        self.log.merge(&leader_election_reply.log);

        let election = &mut self.election.as_mut().unwrap();
        election.insert(from.clone());
        if election.len() >= self.servers.len() / 2 + 1 {
            // won election
            self.election = None;
            // handleNonChosen?
            debug_println(format!("{} elected!", self.leader));
            tokio::spawn(send_liveness_notifications(self.leader.clone()));
        } else {
            debug_println(format!(
                "{} NOT elected, have {} votes",
                self.leader,
                election.len()
            ));
        }

        return Ok(None);
    }

    fn process_liveness_notification(
        &mut self,
        liveness_notification: &PaxosLivenessNotification,
        from: &String,
    ) -> AppResult {
        if from.ne(&liveness_notification.leader.leader) {
            return Err(AppError {
                error_message: format!(
                    "{} tried leader notify for {}",
                    from, &liveness_notification.leader
                ),
            });
        }
        // if message is not for leader, just ignore
        if !self.leader.eq(&liveness_notification.leader) {
            return Ok(None);
        }
        // update last heard from leader timestamp
        self.last_leader_liveness_notification_timestamp = SystemTime::now();

        // not an RPC but update leader on histories
        let knowledge_state_update = PaxosKnowledgeStateUpdate {
            leader: self.leader.clone(),
            map: self.history.clone(),
        };

        let message = Message::from(
            me().clone(),
            from.clone(),
            MessageContent::PaxosKnowledgeStateUpdate(knowledge_state_update),
        );
        debug_println(format!("proc LN from {}", self.leader));
        Ok(Some((message, None)))
    }

    fn process_knowledge_state_update(
        &mut self,
        knowledge_state_update: &PaxosKnowledgeStateUpdate,
        from: &String,
    ) -> AppResult {
        // check that we are in the same leader cycle
        if self.leader.ne(&knowledge_state_update.leader) {
            return Ok(None);
        }
        let other_history = &knowledge_state_update.map;
        for (server, slot_num) in other_history.into_iter() {
            let server_slot_num_record_option = self.history.get_mut(server);
            if let None = server_slot_num_record_option {
                // something wrong!
                continue;
            }
            let server_slot_num_record = server_slot_num_record_option.unwrap();
            if *server_slot_num_record < *slot_num {
                *server_slot_num_record = *slot_num;
            }
        }
        debug_println(format!("proc KSU from {}", from));
        Ok(None)
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

impl App for PaxosApp {
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

async fn send_liveness_notifications(timer_leader: Ballot) {
    let mut message = Message::from(me().clone(), String::new(), MessageContent::Invalid);
    #[cfg(feature = "paxos")]
    loop {
        debug_println(format!("send_liveness_notifications {}", timer_leader));
        let app = APP.lock().await;
        if app.leader.ne(&timer_leader) {
            return;
        }

        message.content = MessageContent::PaxosLivenessNotification(PaxosLivenessNotification {
            leader: app.leader.clone(),
            log: app.log.clone(),
        });

        broadcast(app.servers.iter(), &mut message).await;

        tokio::time::sleep(Duration::from_millis(LIVENESS_NOTIFICATION_DELAY)).await;
    }
}

async fn broadcast<'a, T: Iterator<Item = &'a String>>(servers: T, message: &mut Message) {
    for server in servers {
        if server.eq(me()) {
            continue;
        }
        // edit to field of message, maybe add official modifier on message/make message builder struct
        message.to = server.clone();
        send(&message).await;
    }
}

#[async_recursion]
async fn check_leader_live(leader: Ballot) {
    let mut app = APP.lock().await;
    if app.leader.ne(&leader) {
        return;
    }
    let now = SystemTime::now();
    let duration = Duration::from_millis(LIVENESS_REQUIREMENT_DELAY);
    if now - duration <= app.last_leader_liveness_notification_timestamp {
        debug_println(format!(
            "check_leader_live {}, OK {:?} <= {:?}",
            leader,
            now - duration,
            app.last_leader_liveness_notification_timestamp
        ));
        timer(check_leader_live(leader), duration);
        return;
    }
    debug_println(format!(
        "check_leader_live {}, NOTOK {:?} <= {:?}",
        leader,
        now - duration,
        app.last_leader_liveness_notification_timestamp
    ));

    // start leader election
    app.init_leader_election().await;
}

#[async_recursion]
async fn leader_election_check(mut message: Message, ballot: Ballot) {
    let app = APP.lock().await;
    if app.election.eq(&None) || app.leader.ne(&ballot) {
        return;
    }
    debug_println(format!("leader_election_check rebroadcast for {}", ballot));
    broadcast(app.servers.iter(), &mut message).await;

    timer(
        leader_election_check(message, ballot),
        Duration::from_millis(LEADER_ELECTION_TIMER),
    );
}
