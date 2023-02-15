use std::{
    cmp::{max, min},
    collections::{BTreeMap, VecDeque},
};

use serde::{Deserialize, Serialize};

use crate::{debug_println, me};

use self::slot::{ChosenSlot, Slot, UnchosenSlot};

use super::{
    ballot::Ballot,
    data::Data,
    messages::{PaxosProposeReply, PaxosProposeRequest},
};

pub mod slot;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
pub struct PaxosLog {
    pub slot_offset: usize,
    pub processed_offset: usize,
    pub slots: VecDeque<Slot>,
    pub history: BTreeMap<String, usize>,
    pub num_servers: usize,
}

impl PaxosLog {
    pub fn new<'a, T: Iterator<Item = &'a String>>(servers: T, num_servers: usize) -> Self {
        let mut history = BTreeMap::new();
        for server in servers {
            history.insert(server.clone(), usize::MIN);
        }
        PaxosLog {
            slot_offset: 1,
            processed_offset: 0,
            slots: VecDeque::new(),
            history,
            num_servers,
        }
    }

    pub fn slot(&self, slot_num: usize) -> &Slot {
        if slot_num < self.slot_offset {
            return &Slot::Cleared;
        }
        if slot_num >= self.slot_offset + self.slots.len() {
            return &Slot::Empty;
        }
        let idx: usize = slot_num - self.slot_offset;
        &self.slots.get(idx).unwrap()
    }

    pub(crate) fn merge<'a, T: Clone + Iterator<Item = &'a String>>(
        &mut self,
        other: &PaxosLog,
        servers: T,
    ) {
        if other.slots.is_empty() {
            return;
        }
        let min_slot = min(self.processed_offset, other.processed_offset);
        let max_slot = max(
            self.slot_offset + self.slots.len(),
            other.slot_offset + other.slots.len(),
        );
        for slot_num in min_slot..=max_slot {
            let theirs = other.slots.get(slot_num - other.slot_offset);
            if let None = theirs {
                continue;
            }
            let theirs = theirs.unwrap();
            let mine = self.slots.get_mut(slot_num - self.slot_offset);
            if let None = mine {
                self.add_slot_from_merge(slot_num, theirs);
                continue;
            }
            let mine = mine.unwrap();
            match mine {
                Slot::Chosen(_) => continue,
                Slot::Unchosen(unchosen) => {
                    if unchosen.merge(theirs, servers.clone()) {
                        *mine = Slot::Chosen(unchosen.get_chosen().unwrap());
                    }
                }
                Slot::Empty => todo!(),
                Slot::Cleared => todo!(),
            }
        }

        //todo!()
    }

    fn garbage_collect(&mut self) {
        // update self's history value
        *(self.history.get_mut(me()).unwrap()) = self.processed_offset;

        let min_processed = *self.history.values().min().unwrap();
        for _ in self.slot_offset..=min_processed {
            self.slots.pop_front();
        }
        self.slot_offset = min_processed + 1;
    }

    pub(crate) fn merge_history(&mut self, other_history: &BTreeMap<String, usize>) {
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
        self.garbage_collect();
    }

    fn add_slot_from_merge(&mut self, slot_num: usize, theirs: &Slot) {
        if slot_num < self.processed_offset {
            panic!("tries to add slot already chosen {} {:?}", slot_num, theirs);
        }
        let idx = slot_num - self.slot_offset;
        if (0..self.slots.len()).contains(&idx) {
            let slot = self.slots.get_mut(slot_num).unwrap();
            if let Slot::Empty = slot {
                *slot = theirs.clone()
            } else {
                panic!(
                    "tries to add slot already in there {} mine: {:?} theirs: {:?}",
                    slot_num, slot, theirs
                );
            }
            return;
        }
        for _ in self.slot_offset..slot_num {
            self.slots.push_back(Slot::Empty);
        }
        self.slots.push_back(theirs.clone());
    }

    pub(crate) fn process_propose_request(
        &mut self,
        proposal: &PaxosProposeRequest,
    ) -> Option<PaxosProposeReply> {
        let PaxosProposeRequest { slot, leader, data } = proposal;
        let idx = slot - self.slot_offset;
        if idx < self.slots.len() {
            let log_slot = self.slots.get_mut(idx).unwrap();
            return match log_slot {
                Slot::Chosen(chosen) => Some(PaxosProposeReply {
                    slot: *slot,
                    leader: chosen.leader.clone(),
                    data: chosen.data.clone(),
                    chosen: true,
                }),
                // LOGIC IN LOG SLOT VOTE: try to add or update vote for a log slot that has election.
                // if we have it unchosen then either:
                //  we got it from a log merge
                //      accept this message, check if chosen
                //  it's a repeat message to us:
                //      then recreate reply with accepted value
                //  it's from a new leader
                //      update with new leader and value and check for accepted
                Slot::Unchosen(unchosen) => {
                    let (leader_res, data_res, chosen_res) = unchosen.vote_check(leader, data);
                    return Some(PaxosProposeReply {
                        slot: *slot,
                        leader: leader_res,
                        data: data_res,
                        chosen: chosen_res,
                    });
                }
                Slot::Empty => {
                    let mut new_unchosen = UnchosenSlot::new(self.num_servers);
                    // TODO: stop the clones
                    new_unchosen.new_vote(leader.leader.clone(), leader.clone(), data.clone());
                    new_unchosen.new_vote(me().clone(), leader.clone(), data.clone());

                    // if leader + me was enough to choose i.e. config of 3 servers
                    // then choose
                    if let Some(chosen) = new_unchosen.get_chosen() {
                        *log_slot = Slot::Chosen(chosen.clone());
                        return Some(PaxosProposeReply {
                            slot: *slot,
                            leader: chosen.leader,
                            data: chosen.data,
                            chosen: true,
                        });
                    }

                    *log_slot = Slot::Unchosen(new_unchosen);
                    return Some(PaxosProposeReply {
                        slot: *slot,
                        leader: leader.clone(),
                        data: data.clone(),
                        chosen: false,
                    });
                }
                Slot::Cleared => None,
            };
        }

        // must insert a new slot, append with Empty up to it.
        debug_println(format!("case 1 slot {}", slot));
        for _ in self.slots.len()..idx {
            debug_println("pushing empty");
            self.slots.push_back(Slot::Empty);
        }
        let mut new_unchosen = UnchosenSlot::new(self.num_servers);
        new_unchosen.new_vote(me().clone(), leader.clone(), data.clone());
        new_unchosen.new_vote(leader.leader.clone(), leader.clone(), data.clone());
        self.slots.push_back(Slot::Unchosen(new_unchosen));
        debug_println(format!("after new propose: {:?}", self));

        Some(PaxosProposeReply {
            slot: *slot,
            leader: leader.clone(),
            data: data.clone(),
            chosen: false,
        })
    }

    pub(crate) fn new_slot(&mut self, leader: Ballot, data: Data) -> usize {
        // if config has 1 server, then this is chosen
        if self.num_servers == 1 {
            self.slots
                .push_back(Slot::Chosen(ChosenSlot { leader, data }));
            return self.slots.len() + self.slot_offset - 1;
        }
        // otherwise slot election
        let mut unchosen = UnchosenSlot::new(self.num_servers);
        unchosen.new_vote(me().clone(), leader, data);
        self.slots.push_back(Slot::Unchosen(unchosen));
        self.slots.len() + self.slot_offset - 1
    }

    pub(crate) fn process_propose_reply(
        &mut self,
        propose_reply: &PaxosProposeReply,
        from: &String,
    ) {
        let PaxosProposeReply {
            slot,
            leader,
            data,
            chosen,
        } = propose_reply;
        if !(self.slot_offset..(self.slot_offset + self.slots.len())).contains(slot) {
            return;
        }
        let log_slot = self.slots.get_mut(*slot - self.slot_offset).unwrap();
        let unchosen = match log_slot {
            Slot::Chosen(_) => {
                debug_println(format!("good case 2 slot {} already chosen", slot));
                return;
            }
            Slot::Unchosen(unchosen) => unchosen,
            Slot::Empty => panic!("rec propose reply never made slot?"),
            Slot::Cleared => return,
        };

        if *chosen {
            *log_slot = Slot::Chosen(ChosenSlot {
                leader: leader.clone(),
                data: data.clone(),
            });
            return;
        }

        unchosen.update_vote(from, leader, data);
        if let Some(chosen) = unchosen.get_chosen() {
            *log_slot = Slot::Chosen(chosen);
            debug_println("good case");
        }
    }

    pub(crate) fn is_chosen(&self, slot: usize) -> bool {
        if slot < self.slot_offset {
            return true;
        }
        if slot >= self.slot_offset + self.slots.len() {
            return false;
        }
        let log_slot = self.slots.get(slot - self.slot_offset).unwrap();
        match log_slot {
            Slot::Chosen(_) | Slot::Cleared => true,
            Slot::Unchosen(_) | Slot::Empty => false,
        }
    }
}

pub struct ChosenValue(Vec<u8>);

impl Iterator for PaxosLog {
    type Item = ChosenValue;

    fn next(&mut self) -> Option<Self::Item> {
        let idx: usize = (self.processed_offset - self.slot_offset) as usize;
        if self.slots.len() < idx {
            return None;
        }
        match &self.slots[idx] {
            Slot::Chosen(chosen) => {
                self.processed_offset += 1;
                // this is very bad to clone, need to figure out somehow to not clone
                Some(ChosenValue(chosen.data.data.clone()))
            }
            _ => None,
        }
    }
}
