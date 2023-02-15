use crate::{
    app::paxos::{ballot::Ballot, data::Data},
    me,
};

use self::vote::SlotVote;
use serde::{Deserialize, Serialize};

pub mod vote;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct ChosenSlot {
    pub leader: Ballot,
    pub data: Data,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct UnchosenSlot {
    pub vote: SlotVote,
    pub num_servers: usize,
    chosen: bool,
}
impl UnchosenSlot {
    pub(crate) fn new(num_servers: usize) -> Self {
        UnchosenSlot {
            vote: SlotVote::new(),
            num_servers,
            chosen: false,
        }
    }

    pub(crate) fn merge<'a, T: Iterator<Item = &'a String>>(
        &mut self,
        other: &Slot,
        servers: T,
    ) -> bool {
        if self.chosen {
            return true;
        }
        match other {
            Slot::Chosen(chosen) => {
                self.chosen = true;
                self.vote = SlotVote::won(chosen.data.clone());
                return true;
            }
            Slot::Unchosen(unchosen) => {
                self.vote.merge(&unchosen.vote, servers);
                return self.vote.has_winner(self.num_servers);
            }
            Slot::Empty => return false,
            Slot::Cleared => return false,
        }
    }

    pub(crate) fn get_chosen(&mut self) -> Option<ChosenSlot> {
        let winner_option = self.vote.winner(self.num_servers);
        if let None = winner_option {
            return None;
        }
        let winner = winner_option.unwrap();
        Some(ChosenSlot {
            data: winner.data,
            leader: winner.leader,
        })
    }

    pub(crate) fn new_vote(&mut self, who: String, leader: Ballot, data: Data) {
        self.vote
            .check_no_ret(&who, &leader, &data, self.num_servers);
    }

    pub(crate) fn vote_check(&mut self, leader: &Ballot, data: &Data) -> (Ballot, Data, bool) {
        self.vote
            .check_no_ret(&leader.leader, leader, data, self.num_servers);
        self.vote.check(me(), leader, data, self.num_servers)
    }

    pub(crate) fn update_vote(&mut self, who: &String, leader: &Ballot, data: &Data) {
        self.vote.check_no_ret(who, leader, data, self.num_servers);
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Slot {
    Chosen(ChosenSlot),
    Unchosen(UnchosenSlot),
    Empty,
    Cleared,
}
