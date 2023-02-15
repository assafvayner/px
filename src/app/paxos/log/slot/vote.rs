use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::app::paxos::{ballot::Ballot, data::Data};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct SlotVoteValue {
    pub leader: Ballot,
    pub data: Data,
}

impl SlotVoteValue {
    pub fn new(leader: Ballot, data: Data) -> Self {
        SlotVoteValue { leader, data }
    }
}

impl PartialOrd for SlotVoteValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.leader.partial_cmp(&other.leader)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, PartialOrd, Default)]
pub struct SlotVote {
    pub vote_by_server: BTreeMap<String, SlotVoteValue>,
    pub winner: Option<SlotVoteValue>,
}

impl SlotVote {
    pub fn new() -> Self {
        SlotVote {
            vote_by_server: BTreeMap::new(),
            winner: None,
        }
    }

    pub(crate) fn won(data: Data) -> SlotVote {
        SlotVote {
            vote_by_server: BTreeMap::new(),
            winner: Some(SlotVoteValue {
                leader: Ballot::default(),
                data,
            }),
        }
    }

    pub fn merge<'a, T: Iterator<Item = &'a String>>(&mut self, other: &Self, servers: T) {
        for server in servers {
            let theirs = other.vote_by_server.get(server);
            if let None = theirs {
                continue;
            }
            let theirs = theirs.unwrap();
            let mine = self.vote_by_server.get_mut(server);
            if let None = mine {
                self.vote_by_server
                    .insert(server.to_string(), theirs.clone());
                continue;
            }
            let mine = mine.unwrap();
            if *mine < *theirs {
                *mine = theirs.clone();
            }
        }
    }

    pub fn has_winner(&mut self, num_servers: usize) -> bool {
        if let Some(_) = &self.winner {
            true;
        }
        if self.vote_by_server.len() < num_servers / 2 + 1 {
            return false;
        }

        let mut vote_counts: BTreeMap<u128, usize> = BTreeMap::new();
        for svv in self.vote_by_server.values() {
            let entry = vote_counts.entry(svv.data.id).or_insert(0);
            *entry += 1;
            if *entry >= num_servers / 2 + 1 {
                // TODO: figure out how to not clone
                self.winner = Some(svv.clone());
                return true;
            }
        }
        false
    }

    pub fn winner(&mut self, num_servers: usize) -> Option<SlotVoteValue> {
        if self.has_winner(num_servers) {
            return self.winner.clone();
        }
        None
    }

    pub(crate) fn check(
        &mut self,
        voter: &String,
        leader: &Ballot,
        data: &Data,
        num_servers: usize,
    ) -> (Ballot, Data, bool) {
        let mine = self.vote_by_server.get_mut(voter);
        let leader_res: Ballot;
        let data_res: Data;

        match mine {
            Some(svv) => {
                if svv.leader < *leader {
                    *svv = SlotVoteValue::new(leader.clone(), data.clone());
                    leader_res = leader.clone();
                    data_res = data.clone();
                } else {
                    leader_res = svv.leader.clone();
                    data_res = svv.data.clone();
                }
            }
            None => {
                self.vote_by_server.insert(
                    voter.clone(),
                    SlotVoteValue::new(leader.clone(), data.clone()),
                );
                leader_res = leader.clone();
                data_res = data.clone();
            }
        }

        return (leader_res, data_res, self.has_winner(num_servers));
    }

    // TODO: improve check/check_no_ret, combine
    pub(crate) fn check_no_ret(
        &mut self,
        voter: &String,
        leader: &Ballot,
        data: &Data,
        _num_servers: usize,
    ) {
        let vote = self.vote_by_server.get_mut(voter);

        match vote {
            Some(svv) => {
                if svv.leader < *leader {
                    *svv = SlotVoteValue::new(leader.clone(), data.clone());
                }
            }
            None => {
                self.vote_by_server.insert(
                    voter.clone(),
                    SlotVoteValue::new(leader.clone(), data.clone()),
                );
            }
        }
    }
}
